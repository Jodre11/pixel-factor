package pixelfactor.keycloak;

import java.net.URI;
import java.net.URISyntaxException;
import org.keycloak.Config;

/** Server-wide settings, read once from the {@code spi-authenticator--pixel-factor--*} options. */
public record PluginConfig(URI serviceUrl, String apiToken) {

    /** Validates both settings. Neither may be blank, and the URL must be absolute http or https. */
    public PluginConfig {
        if (serviceUrl == null
                || !serviceUrl.isAbsolute()
                || !("http".equals(serviceUrl.getScheme()) || "https".equals(serviceUrl.getScheme()))) {
            throw new IllegalStateException("pixel-factor: service-url must be an absolute http or https URL");
        }
        if (apiToken == null || apiToken.isBlank()) {
            throw new IllegalStateException("pixel-factor: api-token is required");
        }
    }

    /**
     * Read the settings from the authenticator's SPI scope. Every factory in the jar calls this, so each value is set
     * once, under the authenticator.
     */
    public static PluginConfig load() {
        return from(Config.scope("authenticator", PixelFactorAuthenticatorFactory.ID));
    }

    /** Read the settings from a given scope. */
    public static PluginConfig from(Config.Scope scope) {
        String url = scope.get("service-url");
        if (url == null || url.isBlank()) {
            throw new IllegalStateException("pixel-factor: service-url is required");
        }
        try {
            return new PluginConfig(new URI(url.endsWith("/") ? url : url + "/"), scope.get("api-token"));
        } catch (URISyntaxException e) {
            throw new IllegalStateException("pixel-factor: service-url is not a URL", e);
        }
    }

    @Override
    public String toString() {
        return "PluginConfig[serviceUrl=" + serviceUrl + ", apiToken=<redacted>]";
    }
}
