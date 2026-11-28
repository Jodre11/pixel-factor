package pixelfactor.keycloak;

import jakarta.ws.rs.core.Response;
import java.util.List;
import org.keycloak.forms.login.LoginFormsProvider;

/** Renders {@code pixel-factor-tap.ftl}, the one page used for login and both enrolment entries. */
final class TapPage {

    /** The template, in the jar's theme resources. */
    static final String TEMPLATE = "pixel-factor-tap.ftl";

    /** Which entry the page is for. Chooses the heading and instructions. */
    enum Mode {
        LOGIN,
        ENROL,
        CONFIRM
    }

    private TapPage() {}

    /** The page, or empty if the jar has no file for {@code imageId}, which the caller treats as a deployment error. */
    static java.util.Optional<Response> render(
            LoginFormsProvider form,
            Mode mode,
            String imageId,
            int maxTaps,
            List<String> warnings,
            String error,
            Object... errorParams) {
        java.util.Optional<String> file = Images.fileFor(imageId);
        if (file.isEmpty()) {
            return java.util.Optional.empty();
        }
        form.setAttribute("pfMode", mode.name().toLowerCase(java.util.Locale.ROOT));
        form.setAttribute("pfImageFile", file.get());
        form.setAttribute("pfMaxTaps", maxTaps);
        form.setAttribute("pfFiller", Taps.FILLER);
        form.setAttribute("pfWarnings", warnings);
        if (error != null) {
            form.setError(error, errorParams);
        }
        return java.util.Optional.of(form.createForm(TEMPLATE));
    }
}
