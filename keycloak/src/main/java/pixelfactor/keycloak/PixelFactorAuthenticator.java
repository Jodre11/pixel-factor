package pixelfactor.keycloak;

import jakarta.ws.rs.core.MultivaluedMap;
import jakarta.ws.rs.core.Response;
import java.util.List;
import java.util.Optional;
import org.jboss.logging.Logger;
import org.keycloak.authentication.AuthenticationFlowContext;
import org.keycloak.authentication.AuthenticationFlowError;
import org.keycloak.authentication.Authenticator;
import org.keycloak.events.Errors;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.RealmModel;
import org.keycloak.models.UserModel;
import org.keycloak.sessions.AuthenticationSessionModel;

/** The tap step at login. Runs only after a possession factor; see {@link PossessionGate}. */
public final class PixelFactorAuthenticator implements Authenticator {

    static final String NOTE_IMAGE = "pixel-factor.image";
    static final String NOTE_MAX_TAPS = "pixel-factor.max-taps";
    static final String FIELD_TAPS = "taps";

    private static final Logger LOG = Logger.getLogger(PixelFactorAuthenticator.class);

    private final PixelFactorService service;

    /** An authenticator calling {@code service}. */
    public PixelFactorAuthenticator(PixelFactorService service) {
        this.service = service;
    }

    @Override
    public void authenticate(AuthenticationFlowContext context) {
        if (!gate(context)) {
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
        session.setAuthNote(NOTE_IMAGE, challenge.imageId());
        session.setAuthNote(NOTE_MAX_TAPS, Integer.toString(challenge.maxTaps()));
        page(context, null).ifPresent(context::challenge);
    }

    @Override
    public void action(AuthenticationFlowContext context) {
        if (!gate(context)) {
            return;
        }
        Integer maxTaps = maxTaps(context.getAuthenticationSession());
        if (maxTaps == null) {
            LOG.error("pixel-factor: tap step posted without a challenge");
            context.failure(AuthenticationFlowError.INTERNAL_ERROR);
            return;
        }
        MultivaluedMap<String, String> form = context.getHttpRequest().getDecodedFormParameters();
        List<double[]> taps = Taps.parse(form.getFirst(FIELD_TAPS), maxTaps);
        PixelFactorService.Verification answer;
        try {
            answer = service.verify(context.getUser().getId(), taps);
        } catch (PixelFactorService.ServiceException e) {
            serviceError(context, e);
            return;
        }
        if (answer.match()) {
            context.success();
            return;
        }
        // Telling the user about a lock is safe: they have passed the possession factor, and the service sets the
        // flag for unknown ids too.
        String message = answer.locked() ? "pixelFactorLocked" : "pixelFactorNoMatch";
        context.getEvent().user(context.getUser()).error(Errors.INVALID_USER_CREDENTIALS);
        page(context, message)
                .ifPresent(response -> context.failureChallenge(AuthenticationFlowError.INVALID_CREDENTIALS, response));
    }

    private boolean gate(AuthenticationFlowContext context) {
        if (PossessionGate.passed(context.getAuthenticationSession(), context.getRealm())) {
            return true;
        }
        LOG.error("pixel-factor: tap step reached without a possession factor; check the flow puts a passkey or OTP"
                + " step first. Refusing.");
        context.failure(AuthenticationFlowError.ACCESS_DENIED);
        return false;
    }

    /** The tap page, or empty after failing the flow if the jar has no file for the image. */
    private Optional<Response> page(AuthenticationFlowContext context, String error) {
        AuthenticationSessionModel session = context.getAuthenticationSession();
        String imageId = session.getAuthNote(NOTE_IMAGE);
        Integer maxTaps = maxTaps(session);
        Optional<Response> page = maxTaps == null
                ? Optional.empty()
                : TapPage.render(context.form(), TapPage.Mode.LOGIN, imageId, maxTaps, List.of(), error);
        if (page.isEmpty()) {
            LOG.error("pixel-factor: no image file in the plugin for the image the service chose");
            context.failure(AuthenticationFlowError.INTERNAL_ERROR);
        }
        return page;
    }

    static Integer maxTaps(AuthenticationSessionModel session) {
        String note = session.getAuthNote(NOTE_MAX_TAPS);
        if (note == null) {
            return null;
        }
        try {
            int value = Integer.parseInt(note);
            return value > 0 && value <= 32 ? value : null;
        } catch (NumberFormatException e) {
            return null;
        }
    }

    private static void serviceError(AuthenticationFlowContext context, PixelFactorService.ServiceException e) {
        LOG.errorf("pixel-factor: service call failed: %s", e.getMessage());
        context.failure(AuthenticationFlowError.INTERNAL_ERROR);
    }

    @Override
    public boolean requiresUser() {
        return true;
    }

    @Override
    public boolean configuredFor(KeycloakSession session, RealmModel realm, UserModel user) {
        return PixelFactorCredentialProvider.marker(user).isPresent();
    }

    @Override
    public void setRequiredActions(KeycloakSession session, RealmModel realm, UserModel user) {
        user.addRequiredAction(PixelFactorEnrolActionFactory.ID);
    }

    @Override
    public void close() {}
}
