package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

import org.junit.jupiter.api.Test;
import org.keycloak.Config;

class PluginConfigTest {

    private static Config.Scope scope(String url, String token) {
        Config.Scope scope = mock(Config.Scope.class);
        when(scope.get("service-url")).thenReturn(url);
        when(scope.get("api-token")).thenReturn(token);
        return scope;
    }

    @Test
    void readsBothSettingsAndEndsTheUrlWithASlash() {
        PluginConfig config = PluginConfig.from(scope("http://service:8080", "secret"));
        assertEquals("http://service:8080/", config.serviceUrl().toString());
        assertEquals(
                "http://service:8080/v1/verify",
                config.serviceUrl().resolve("v1/verify").toString());
        assertEquals("secret", config.apiToken());
    }

    @Test
    void refusesMissingOrBadSettings() {
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope(null, "t")));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope(" ", "t")));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope("http://s", null)));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope("http://s", "")));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope("file:///etc/passwd", "t")));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope("service:8080", "t")));
        assertThrows(IllegalStateException.class, () -> PluginConfig.from(scope("http://bad host", "t")));
    }

    @Test
    void neverPrintsTheToken() {
        assertFalse(PluginConfig.from(scope("http://s", "secret")).toString().contains("secret"));
    }
}
