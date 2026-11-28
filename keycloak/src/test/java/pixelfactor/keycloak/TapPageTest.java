package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.RETURNS_SELF;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import jakarta.ws.rs.core.Response;
import java.util.List;
import java.util.Optional;
import org.junit.jupiter.api.Test;
import org.keycloak.forms.login.LoginFormsProvider;

class TapPageTest {

    @Test
    void imageIdsResolveOnlyToFilesInTheJar() {
        assertEquals(Optional.of("demo.svg"), Images.fileFor("demo"));
        assertTrue(Images.fileFor(null).isEmpty());
        assertTrue(Images.fileFor("").isEmpty());
        assertTrue(Images.fileFor("unknown").isEmpty());
        assertTrue(Images.fileFor("../templates/pixel-factor-tap").isEmpty(), "traversal");
        assertTrue(Images.fileFor("img/demo").isEmpty(), "separator");
        assertTrue(Images.fileFor("demo.svg").isEmpty(), "extension is not part of the id");
        assertTrue(Images.fileFor("a".repeat(65)).isEmpty(), "too long");
    }

    @Test
    void rendersTheTemplateWithTheAttributesThePageNeeds() {
        LoginFormsProvider form = mock(LoginFormsProvider.class, RETURNS_SELF);
        Response page = mock(Response.class);
        when(form.createForm(TapPage.TEMPLATE)).thenReturn(page);
        Optional<Response> result =
                TapPage.render(form, TapPage.Mode.CONFIRM, "demo", 8, List.of("close"), "pixelFactorRejected", "why");
        assertEquals(Optional.of(page), result);
        verify(form).setAttribute("pfMode", "confirm");
        verify(form).setAttribute("pfImageFile", "demo.svg");
        verify(form).setAttribute("pfMaxTaps", 8);
        verify(form).setAttribute("pfFiller", Taps.FILLER);
        verify(form).setAttribute("pfWarnings", List.of("close"));
        verify(form).setError("pixelFactorRejected", "why");
    }

    @Test
    void anImageTheJarLacksRendersNothing() {
        LoginFormsProvider form = mock(LoginFormsProvider.class, RETURNS_SELF);
        assertTrue(TapPage.render(form, TapPage.Mode.LOGIN, "nope", 8, List.of(), null)
                .isEmpty());
        verify(form, never()).createForm(anyString());
        verify(form, never()).setAttribute(anyString(), any());
    }
}
