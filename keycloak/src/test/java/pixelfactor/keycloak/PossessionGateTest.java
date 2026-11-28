package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

import java.util.Map;
import org.junit.jupiter.api.Test;
import org.keycloak.models.AuthenticationExecutionModel;
import org.keycloak.models.RealmModel;
import org.keycloak.sessions.AuthenticationSessionModel;
import org.keycloak.sessions.CommonClientSessionModel.ExecutionStatus;

class PossessionGateTest {

    private final RealmModel realm = mock(RealmModel.class);
    private final AuthenticationSessionModel session = mock(AuthenticationSessionModel.class);

    private void execution(String id, String provider) {
        AuthenticationExecutionModel model = new AuthenticationExecutionModel();
        model.setId(id);
        model.setAuthenticator(provider);
        when(realm.getAuthenticationExecutionById(id)).thenReturn(model);
    }

    @Test
    void passesOnASuccessfulOtpOrPasskey() {
        execution("e1", "auth-username-form");
        execution("e2", "auth-otp-form");
        when(session.getExecutionStatus())
                .thenReturn(Map.of("e1", ExecutionStatus.SUCCESS, "e2", ExecutionStatus.SUCCESS));
        assertTrue(PossessionGate.passed(session, realm));

        execution("e3", "webauthn-authenticator-passwordless");
        when(session.getExecutionStatus()).thenReturn(Map.of("e3", ExecutionStatus.SUCCESS));
        assertTrue(PossessionGate.passed(session, realm));
    }

    @Test
    void failsClosedOnEverythingElse() {
        execution("u", "auth-username-form");
        execution("p", "auth-username-password-form");
        execution("c", "auth-cookie");
        execution("o", "auth-otp-form");
        when(session.getExecutionStatus())
                .thenReturn(Map.of(
                        "u", ExecutionStatus.SUCCESS, "p", ExecutionStatus.SUCCESS, "c", ExecutionStatus.SUCCESS));
        assertFalse(PossessionGate.passed(session, realm), "no possession factor");

        for (ExecutionStatus status : ExecutionStatus.values()) {
            if (status == ExecutionStatus.SUCCESS) {
                continue;
            }
            when(session.getExecutionStatus()).thenReturn(Map.of("o", status));
            assertFalse(PossessionGate.passed(session, realm), status.name());
        }

        when(session.getExecutionStatus()).thenReturn(Map.of("gone", ExecutionStatus.SUCCESS));
        assertFalse(PossessionGate.passed(session, realm), "unknown execution");

        execution("subflow", null);
        when(session.getExecutionStatus()).thenReturn(Map.of("subflow", ExecutionStatus.SUCCESS));
        assertFalse(PossessionGate.passed(session, realm), "a subflow has no authenticator");

        when(session.getExecutionStatus()).thenReturn(null);
        assertFalse(PossessionGate.passed(session, realm), "no statuses");
    }
}
