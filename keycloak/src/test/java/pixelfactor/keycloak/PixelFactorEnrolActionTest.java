package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.RETURNS_SELF;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.verifyNoInteractions;
import static org.mockito.Mockito.when;

import jakarta.ws.rs.core.MultivaluedHashMap;
import jakarta.ws.rs.core.Response;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.keycloak.authentication.RequiredActionContext;
import org.keycloak.credential.CredentialModel;
import org.keycloak.forms.login.LoginFormsProvider;
import org.keycloak.http.HttpRequest;
import org.keycloak.models.AuthenticationExecutionModel;
import org.keycloak.models.RealmModel;
import org.keycloak.models.SubjectCredentialManager;
import org.keycloak.models.UserModel;
import org.keycloak.sessions.AuthenticationSessionModel;
import org.keycloak.sessions.CommonClientSessionModel.ExecutionStatus;

class PixelFactorEnrolActionTest {

    private final PixelFactorService service = mock(PixelFactorService.class);
    private final RequiredActionContext context = mock(RequiredActionContext.class);
    private final AuthenticationSessionModel session = mock(AuthenticationSessionModel.class);
    private final RealmModel realm = mock(RealmModel.class);
    private final UserModel user = mock(UserModel.class);
    private final SubjectCredentialManager credentials = mock(SubjectCredentialManager.class);
    private final LoginFormsProvider form = mock(LoginFormsProvider.class, RETURNS_SELF);
    private final HttpRequest request = mock(HttpRequest.class);
    private final MultivaluedHashMap<String, String> posted = new MultivaluedHashMap<>();
    private final Response page = mock(Response.class);
    private final Map<String, String> notes = new HashMap<>();
    private final PixelFactorEnrolAction action = new PixelFactorEnrolAction(service);
    private final List<double[]> taps = List.of(new double[] {0.1, 0.2}, new double[] {0.3, 0.4});

    @BeforeEach
    void wire() {
        when(context.getAuthenticationSession()).thenReturn(session);
        when(context.getRealm()).thenReturn(realm);
        when(context.getUser()).thenReturn(user);
        when(context.form()).thenReturn(form);
        when(context.getHttpRequest()).thenReturn(request);
        when(request.getDecodedFormParameters()).thenReturn(posted);
        when(form.createForm(anyString())).thenReturn(page);
        when(user.getId()).thenReturn("uuid-1");
        when(user.credentialManager()).thenReturn(credentials);
        when(session.getAuthNote(anyString())).thenAnswer(i -> notes.get(i.getArgument(0, String.class)));
        org.mockito.Mockito.doAnswer(i -> notes.put(i.getArgument(0), i.getArgument(1)))
                .when(session)
                .setAuthNote(anyString(), anyString());
        org.mockito.Mockito.doAnswer(i -> notes.remove(i.getArgument(0, String.class)))
                .when(session)
                .removeAuthNote(anyString());
        AuthenticationExecutionModel otp = new AuthenticationExecutionModel();
        otp.setAuthenticator("auth-otp-form");
        when(realm.getAuthenticationExecutionById("otp")).thenReturn(otp);
        when(session.getExecutionStatus()).thenReturn(Map.of("otp", ExecutionStatus.SUCCESS));
    }

    private void start() throws Exception {
        when(service.challenge("uuid-1")).thenReturn(new PixelFactorService.Challenge("demo", 8));
        action.requiredActionChallenge(context);
        posted.putSingle("taps", Taps.format(taps, 8));
    }

    @Test
    void aSessionWithoutAPossessionFactorCannotEnrol() {
        when(session.getExecutionStatus()).thenReturn(Map.of("otp", ExecutionStatus.CHALLENGED));
        action.requiredActionChallenge(context);
        action.processAction(context);
        verify(context, org.mockito.Mockito.times(2)).failure("pixelFactorPossessionRequired");
        verifyNoInteractions(service);
    }

    @Test
    void stagesThenConfirmsThenWritesTheMarker() throws Exception {
        start();
        when(service.stageEnrolment(eq("uuid-1"), eq("demo"), any()))
                .thenReturn(PixelFactorService.Staged.accepted(List.of()));
        action.processAction(context);
        assertEquals(PixelFactorEnrolAction.STEP_CONFIRM, notes.get(PixelFactorEnrolAction.NOTE_STEP));
        verify(credentials, never()).createStoredCredential(any());

        when(service.confirmEnrolment(eq("uuid-1"), any())).thenReturn(true);
        when(credentials.getStoredCredentialsByTypeStream(PixelFactorCredentialProvider.TYPE))
                .thenReturn(Stream.empty());
        action.processAction(context);
        verify(credentials).createStoredCredential(any());
        verify(context).success();
    }

    @Test
    void reEnrolmentUpdatesTheMarkerInPlace() throws Exception {
        start();
        notes.put(PixelFactorEnrolAction.NOTE_STEP, PixelFactorEnrolAction.STEP_CONFIRM);
        CredentialModel existing = new CredentialModel();
        when(credentials.getStoredCredentialsByTypeStream(PixelFactorCredentialProvider.TYPE))
                .thenReturn(Stream.of(existing));
        when(service.confirmEnrolment(eq("uuid-1"), any())).thenReturn(true);
        action.processAction(context);
        verify(credentials).updateStoredCredential(existing);
        verify(credentials, never()).createStoredCredential(any());
        verify(credentials, never()).removeStoredCredentialById(anyString());
        assertEquals("{\"image_id\":\"demo\"}", existing.getCredentialData());
    }

    @Test
    void aRejectedSequenceAsksAgain() throws Exception {
        start();
        when(service.stageEnrolment(eq("uuid-1"), eq("demo"), any()))
                .thenReturn(PixelFactorService.Staged.rejected("too few taps"));
        action.processAction(context);
        verify(form).setError("pixelFactorRejected", "too few taps");
        assertEquals(PixelFactorEnrolAction.STEP_ENROL, notes.get(PixelFactorEnrolAction.NOTE_STEP));
    }

    @Test
    void aWrongConfirmationStartsAgainWithoutAMarker() throws Exception {
        start();
        notes.put(PixelFactorEnrolAction.NOTE_STEP, PixelFactorEnrolAction.STEP_CONFIRM);
        when(service.confirmEnrolment(eq("uuid-1"), any())).thenReturn(false);
        action.processAction(context);
        verify(form).setError("pixelFactorConfirmMismatch");
        assertEquals(PixelFactorEnrolAction.STEP_ENROL, notes.get(PixelFactorEnrolAction.NOTE_STEP));
        verify(credentials, never()).createStoredCredential(any());
        verify(context, never()).success();
    }

    @Test
    void aServiceFailureLeavesTheActionPending() throws Exception {
        start();
        notes.put(PixelFactorEnrolAction.NOTE_STEP, PixelFactorEnrolAction.STEP_CONFIRM);
        when(service.confirmEnrolment(eq("uuid-1"), any())).thenThrow(new PixelFactorService.ServiceException("down"));
        action.processAction(context);
        verify(context).failure("pixelFactorServiceError");
        verify(context, never()).success();
        verify(credentials, never()).createStoredCredential(any());
    }

    @Test
    void anApplicationInitiatedActionAlwaysNeedsAFreshLogin() {
        assertEquals(0, action.getMaxAuthAge(null));
    }
}
