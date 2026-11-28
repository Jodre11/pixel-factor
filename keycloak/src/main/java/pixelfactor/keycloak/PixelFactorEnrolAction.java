package pixelfactor.keycloak;

import jakarta.ws.rs.core.MultivaluedMap;
import jakarta.ws.rs.core.Response;
import java.util.List;
import java.util.Optional;
import org.jboss.logging.Logger;
import org.keycloak.authentication.InitiatedActionSupport;
import org.keycloak.authentication.RequiredActionContext;
import org.keycloak.authentication.RequiredActionProvider;
import org.keycloak.models.KeycloakSession;
import org.keycloak.sessions.AuthenticationSessionModel;

/**
 * Enrolment and re-enrolment, as a required action.
 *
 * <p>The first entry is staged at the service and the second confirms it, so a new sequence replaces the old one only
 * once the user has repeated it. Nothing about the taps is kept in Keycloak between the two requests. The marker
 * credential is written only after a confirmation.
 */
public final class PixelFactorEnrolAction implements RequiredActionProvider {

    static final String NOTE_STEP = "pixel-factor.enrol-step";
    static final String NOTE_WARNINGS = "pixel-factor.warnings";
    static final String STEP_ENROL = "enrol";
    static final String STEP_CONFIRM = "confirm";
    static final String FIELD_RESTART = "restart";

    private static final Logger LOG = Logger.getLogger(PixelFactorEnrolAction.class);

    private final PixelFactorService service;

    /** An action calling {@code service}. */
    public PixelFactorEnrolAction(PixelFactorService service) {
        this.service = service;
    }

    @Override
    public InitiatedActionSupport initiatedActionSupport() {
        return InitiatedActionSupport.SUPPORTED;
    }

    /**
     * Zero, whatever the realm configures: started from the account console, Keycloak makes the user log in afresh,
     * so the possession factor succeeds in the session the gate checks.
     */
    @Override
    public int getMaxAuthAge(KeycloakSession session) {
        return 0;
    }

    @Override
    public void evaluateTriggers(RequiredActionContext context) {
        // Set by the tap step's setRequiredActions for users who have not enrolled.
    }

    @Override
    public void requiredActionChallenge(RequiredActionContext context) {
        if (!requirePossession(context)) {
            return;
        }
        PixelFactorService.Challenge challenge;
        try {
            challenge = service.challenge(context.getUser().getId());
        } catch (PixelFactorService.ServiceException e) {
            serviceError(context, e);
            return;
        }
        AuthenticationSessionModel session = context.getAuthenticationSession();
        session.setAuthNote(PixelFactorAuthenticator.NOTE_IMAGE, challenge.imageId());
        session.setAuthNote(PixelFactorAuthenticator.NOTE_MAX_TAPS, Integer.toString(challenge.maxTaps()));
        session.setAuthNote(NOTE_STEP, STEP_ENROL);
        showOrFail(context, TapPage.Mode.ENROL, List.of(), null);
    }

    @Override
    public void processAction(RequiredActionContext context) {
        if (!requirePossession(context)) {
            return;
        }
        AuthenticationSessionModel session = context.getAuthenticationSession();
        Integer maxTaps = PixelFactorAuthenticator.maxTaps(session);
        String imageId = session.getAuthNote(PixelFactorAuthenticator.NOTE_IMAGE);
        if (maxTaps == null || imageId == null) {
            requiredActionChallenge(context);
            return;
        }
        MultivaluedMap<String, String> form = context.getHttpRequest().getDecodedFormParameters();
        if (form.containsKey(FIELD_RESTART)) {
            session.setAuthNote(NOTE_STEP, STEP_ENROL);
            showOrFail(context, TapPage.Mode.ENROL, List.of(), null);
            return;
        }
        List<double[]> taps = Taps.parse(form.getFirst(PixelFactorAuthenticator.FIELD_TAPS), maxTaps);
        String userId = context.getUser().getId();
        try {
            if (STEP_CONFIRM.equals(session.getAuthNote(NOTE_STEP))) {
                confirm(context, userId, imageId, taps);
            } else {
                stage(context, userId, imageId, taps);
            }
        } catch (PixelFactorService.ServiceException e) {
            serviceError(context, e);
        }
    }

    private void stage(RequiredActionContext context, String userId, String imageId, List<double[]> taps)
            throws PixelFactorService.ServiceException {
        PixelFactorService.Staged staged = service.stageEnrolment(userId, imageId, taps);
        if (!staged.accepted()) {
            showOrFail(context, TapPage.Mode.ENROL, List.of(), "pixelFactorRejected", staged.reason());
            return;
        }
        AuthenticationSessionModel session = context.getAuthenticationSession();
        session.setAuthNote(NOTE_STEP, STEP_CONFIRM);
        session.setAuthNote(NOTE_WARNINGS, String.join("\n", staged.warnings()));
        showOrFail(context, TapPage.Mode.CONFIRM, staged.warnings(), null);
    }

    private void confirm(RequiredActionContext context, String userId, String imageId, List<double[]> taps)
            throws PixelFactorService.ServiceException {
        AuthenticationSessionModel session = context.getAuthenticationSession();
        if (!service.confirmEnrolment(userId, taps)) {
            // The service discards the pending enrolment on a wrong confirmation: start again.
            session.setAuthNote(NOTE_STEP, STEP_ENROL);
            showOrFail(context, TapPage.Mode.ENROL, List.of(), "pixelFactorConfirmMismatch");
            return;
        }
        PixelFactorCredentialProvider.recordEnrolment(context.getUser(), imageId);
        session.removeAuthNote(NOTE_STEP);
        session.removeAuthNote(NOTE_WARNINGS);
        context.success();
    }

    /** Show the tap page, or fail the action if it cannot be built. */
    private void showOrFail(
            RequiredActionContext context,
            TapPage.Mode mode,
            List<String> warnings,
            String error,
            Object... errorParams) {
        AuthenticationSessionModel session = context.getAuthenticationSession();
        Integer maxTaps = PixelFactorAuthenticator.maxTaps(session);
        String imageId = session.getAuthNote(PixelFactorAuthenticator.NOTE_IMAGE);
        if (maxTaps == null) {
            LOG.error("pixel-factor: missing or invalid max-taps note in the authentication session");
            context.failure("pixelFactorServiceError");
            return;
        }
        Optional<Response> page = TapPage.render(context.form(), mode, imageId, maxTaps, warnings, error, errorParams);
        if (page.isEmpty()) {
            LOG.error("pixel-factor: no image file in the plugin for the image the service chose");
            context.failure("pixelFactorServiceError");
            return;
        }
        context.challenge(page.get());
    }

    /** True if a possession factor passed; otherwise fails the action and returns false. */
    private static boolean requirePossession(RequiredActionContext context) {
        if (PossessionGate.passed(context.getAuthenticationSession(), context.getRealm())) {
            return true;
        }
        LOG.error("pixel-factor: enrolment reached without a possession factor in this login. Refusing.");
        context.failure("pixelFactorPossessionRequired");
        return false;
    }

    private static void serviceError(RequiredActionContext context, PixelFactorService.ServiceException e) {
        LOG.errorf("pixel-factor: service call failed: %s", e.getMessage());
        context.failure("pixelFactorServiceError");
    }

    @Override
    public void close() {}
}
