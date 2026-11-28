package pixelfactor.keycloak;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.RETURNS_SELF;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import jakarta.ws.rs.core.MultivaluedHashMap;
import jakarta.ws.rs.core.Response;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.keycloak.authentication.AuthenticationFlowContext;
import org.keycloak.authentication.AuthenticationFlowError;
import org.keycloak.events.EventBuilder;
import org.keycloak.forms.login.LoginFormsProvider;
import org.keycloak.http.HttpRequest;
import org.keycloak.models.AuthenticationExecutionModel;
import org.keycloak.models.RealmModel;
import org.keycloak.models.UserModel;
import org.keycloak.sessions.AuthenticationSessionModel;
import org.keycloak.sessions.CommonClientSessionModel.ExecutionStatus;

class PixelFactorAuthenticatorTest {

    private final PixelFactorService service = mock(PixelFactorService.class);
    private final AuthenticationFlowContext context = mock(AuthenticationFlowContext.class);
    private final AuthenticationSessionModel session = mock(AuthenticationSessionModel.class);
    private final RealmModel realm = mock(RealmModel.class);
    private final UserModel user = mock(UserModel.class);
    private final LoginFormsProvider form = mock(LoginFormsProvider.class, RETURNS_SELF);
    private final HttpRequest request = mock(HttpRequest.class);
    private final MultivaluedHashMap<String, String> posted = new MultivaluedHashMap<>();
    private final Response page = mock(Response.class);
    private final PixelFactorAuthenticator authenticator = new PixelFactorAuthenticator(service);

    @BeforeEach
    void wire() {
        when(context.getAuthenticationSession()).thenReturn(session);
        when(context.getRealm()).thenReturn(realm);
        when(context.getUser()).thenReturn(user);
        when(context.form()).thenReturn(form);
        when(context.getHttpRequest()).thenReturn(request);
        when(context.getEvent()).thenReturn(mock(EventBuilder.class, RETURNS_SELF));
        when(request.getDecodedFormParameters()).thenReturn(posted);
        when(form.createForm(anyString())).thenReturn(page);
        when(user.getId()).thenReturn("uuid-1");
        AuthenticationExecutionModel otp = new AuthenticationExecutionModel();
        otp.setAuthenticator("auth-otp-form");
        when(realm.getAuthenticationExecutionById("otp")).thenReturn(otp);
    }

    private void possession(boolean passed) {
        when(session.getExecutionStatus())
                .thenReturn(Map.of("otp", passed ? ExecutionStatus.SUCCESS : ExecutionStatus.CHALLENGED));
    }

    private void challenged() {
        when(session.getAuthNote(PixelFactorAuthenticator.NOTE_IMAGE)).thenReturn("demo");
        when(session.getAuthNote(PixelFactorAuthenticator.NOTE_MAX_TAPS)).thenReturn("8");
    }

    @Test
    void withoutAPossessionFactorNothingReachesTheService() throws Exception {
        possession(false);
        authenticator.authenticate(context);
        authenticator.action(context);
        verify(context, org.mockito.Mockito.times(2)).failure(AuthenticationFlowError.ACCESS_DENIED);
        org.mockito.Mockito.verifyNoInteractions(service);
    }

    @Test
    void authenticateShowsTheServicesImage() throws Exception {
        possession(true);
        when(service.challenge("uuid-1")).thenReturn(new PixelFactorService.Challenge("demo", 8));
        challenged();
        authenticator.authenticate(context);
        verify(session).setAuthNote(PixelFactorAuthenticator.NOTE_IMAGE, "demo");
        verify(form).setAttribute("pfImageFile", "demo.svg");
        verify(form).setAttribute("pfMaxTaps", 8);
        verify(context).challenge(page);
    }

    @Test
    void anImageTheJarLacksIsAnInternalError() throws Exception {
        possession(true);
        when(service.challenge("uuid-1")).thenReturn(new PixelFactorService.Challenge("nope", 8));
        when(session.getAuthNote(PixelFactorAuthenticator.NOTE_IMAGE)).thenReturn("nope");
        when(session.getAuthNote(PixelFactorAuthenticator.NOTE_MAX_TAPS)).thenReturn("8");
        authenticator.authenticate(context);
        verify(context).failure(AuthenticationFlowError.INTERNAL_ERROR);
        verify(context, never()).challenge(any());
    }

    @Test
    void aMatchSucceeds() throws Exception {
        possession(true);
        challenged();
        posted.putSingle("taps", Taps.format(List.of(new double[] {0.1, 0.2}), 8));
        when(service.verify(eq("uuid-1"), any())).thenReturn(new PixelFactorService.Verification(true, false));
        authenticator.action(context);
        verify(context).success();
    }

    @Test
    void malformedTapsAreSentAsAnEmptySequence() throws Exception {
        possession(true);
        challenged();
        posted.putSingle("taps", "garbage");
        when(service.verify(eq("uuid-1"), any())).thenReturn(new PixelFactorService.Verification(false, false));
        authenticator.action(context);
        verify(service).verify(eq("uuid-1"), eq(List.of()));
        verify(form).setError("pixelFactorNoMatch");
        verify(context).failureChallenge(AuthenticationFlowError.INVALID_CREDENTIALS, page);
    }

    @Test
    void aLockIsReported() throws Exception {
        possession(true);
        challenged();
        when(service.verify(eq("uuid-1"), any())).thenReturn(new PixelFactorService.Verification(false, true));
        authenticator.action(context);
        verify(form).setError("pixelFactorLocked");
        verify(context).failureChallenge(AuthenticationFlowError.INVALID_CREDENTIALS, page);
    }

    @Test
    void aServiceFailureIsAnInternalErrorNeverSuccess() throws Exception {
        possession(true);
        challenged();
        when(service.verify(eq("uuid-1"), any())).thenThrow(new PixelFactorService.ServiceException("down"));
        authenticator.action(context);
        verify(context).failure(AuthenticationFlowError.INTERNAL_ERROR);
        verify(context, never()).success();
    }

    @Test
    void aPostWithoutAChallengeIsAnInternalError() throws Exception {
        possession(true);
        authenticator.action(context);
        verify(context).failure(AuthenticationFlowError.INTERNAL_ERROR);
        org.mockito.Mockito.verifyNoInteractions(service);
    }
}
