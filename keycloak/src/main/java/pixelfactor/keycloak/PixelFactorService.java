package pixelfactor.keycloak;

import java.util.List;

/** The calls the plugin makes to pixel-factor-service. See {@code docs/service.md}. */
public interface PixelFactorService {

    /** The image to show a user, and the fixed number of tap slots to submit. */
    record Challenge(String imageId, int maxTaps) {}

    /** The answer to a verification. */
    record Verification(boolean match, boolean locked) {}

    /** The answer to staging an enrolment: accepted with warnings, or rejected with a reason for the user. */
    record Staged(boolean accepted, List<String> warnings, String reason) {

        /** An accepted enrolment. */
        public static Staged accepted(List<String> warnings) {
            return new Staged(true, List.copyOf(warnings), null);
        }

        /** A sequence the policy does not accept. */
        public static Staged rejected(String reason) {
            return new Staged(false, List.of(), reason);
        }
    }

    /** {@code POST /v1/challenge}. */
    Challenge challenge(String userId) throws ServiceException;

    /** {@code POST /v1/enrolments}. */
    Staged stageEnrolment(String userId, String imageId, List<double[]> taps) throws ServiceException;

    /** {@code POST /v1/enrolments/confirm}. */
    boolean confirmEnrolment(String userId, List<double[]> taps) throws ServiceException;

    /** {@code POST /v1/verify}. */
    Verification verify(String userId, List<double[]> taps) throws ServiceException;

    /** {@code DELETE /v1/users/{id}}. */
    void delete(String userId) throws ServiceException;

    /** Anything other than a documented answer: transport failure, timeout or an unexpected status. */
    final class ServiceException extends Exception {
        private static final long serialVersionUID = 1L;

        /** A failure with a message for the log, never for the user. */
        public ServiceException(String message) {
            super(message);
        }

        /** A failure caused by another exception. */
        public ServiceException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
