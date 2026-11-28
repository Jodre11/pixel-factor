package pixelfactor.keycloak;

import java.util.List;
import org.keycloak.Config;
import org.keycloak.authentication.Authenticator;
import org.keycloak.authentication.AuthenticatorFactory;
import org.keycloak.models.AuthenticationExecutionModel;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.KeycloakSessionFactory;
import org.keycloak.provider.ProviderConfigProperty;

/** Registers the tap step, provider id {@value #ID}. */
public final class PixelFactorAuthenticatorFactory implements AuthenticatorFactory {

    /** The provider id, also the SPI configuration scope for every setting in the jar. */
    public static final String ID = "pixel-factor";

    private PixelFactorService service;

    @Override
    public String getId() {
        return ID;
    }

    @Override
    public void init(Config.Scope scope) {
        // Fails Keycloak at start when the URL or token is missing, rather than coming up half working.
        service = new HttpServiceClient(PluginConfig.from(scope));
    }

    @Override
    public void postInit(KeycloakSessionFactory factory) {}

    @Override
    public Authenticator create(KeycloakSession session) {
        return new PixelFactorAuthenticator(service);
    }

    @Override
    public String getDisplayType() {
        return "Pixel factor tap step";
    }

    @Override
    public String getReferenceCategory() {
        return PixelFactorCredentialProvider.TYPE;
    }

    @Override
    public boolean isConfigurable() {
        return false;
    }

    @Override
    public AuthenticationExecutionModel.Requirement[] getRequirementChoices() {
        return new AuthenticationExecutionModel.Requirement[] {
            AuthenticationExecutionModel.Requirement.REQUIRED, AuthenticationExecutionModel.Requirement.DISABLED
        };
    }

    @Override
    public boolean isUserSetupAllowed() {
        return true;
    }

    @Override
    public String getHelpText() {
        return "Asks for the user's tap sequence. Must come after a passkey or one-time code step.";
    }

    @Override
    public List<ProviderConfigProperty> getConfigProperties() {
        return List.of();
    }

    @Override
    public void close() {}
}
