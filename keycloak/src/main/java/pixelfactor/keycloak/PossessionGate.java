package pixelfactor.keycloak;

import java.util.Map;
import java.util.Set;
import org.keycloak.models.AuthenticationExecutionModel;
import org.keycloak.models.RealmModel;
import org.keycloak.sessions.AuthenticationSessionModel;
import org.keycloak.sessions.CommonClientSessionModel.ExecutionStatus;

/**
 * Whether a possession factor has already succeeded in this login.
 *
 * <p>The tap step and enrolment both refuse to run without one. Otherwise anyone who knows a username could record
 * failed attempts, and so use the service's lockout to keep the user out, or enrol taps on an account that is not
 * theirs. See {@code docs/keycloak.md}, "The possession gate".
 */
public final class PossessionGate {

    /**
     * Keycloak's provider ids for the possession factors. Fixed in code: a setting would let an operator add a provider
     * that is not a possession factor and quietly defeat the gate.
     */
    public static final Set<String> POSSESSION_PROVIDERS =
            Set.of("webauthn-authenticator", "webauthn-authenticator-passwordless", "auth-otp-form");

    private PossessionGate() {}

    /** True only if an execution of a possession provider has status {@code SUCCESS} in this session. */
    public static boolean passed(AuthenticationSessionModel session, RealmModel realm) {
        Map<String, ExecutionStatus> statuses = session.getExecutionStatus();
        if (statuses == null) {
            return false;
        }
        for (Map.Entry<String, ExecutionStatus> entry : statuses.entrySet()) {
            if (entry.getValue() != ExecutionStatus.SUCCESS) {
                continue;
            }
            AuthenticationExecutionModel execution = realm.getAuthenticationExecutionById(entry.getKey());
            // A subflow's execution has no authenticator, and Set.of rejects a null lookup.
            String provider = execution == null ? null : execution.getAuthenticator();
            if (provider != null && POSSESSION_PROVIDERS.contains(provider)) {
                return true;
            }
        }
        return false;
    }
}
