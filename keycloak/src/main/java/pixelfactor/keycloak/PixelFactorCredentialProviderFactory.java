package pixelfactor.keycloak;

import org.keycloak.Config;
import org.keycloak.credential.CredentialProvider;
import org.keycloak.credential.CredentialProviderFactory;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.KeycloakSessionFactory;

/** Registers {@link PixelFactorCredentialProvider}. */
public final class PixelFactorCredentialProviderFactory
        implements CredentialProviderFactory<PixelFactorCredentialProvider> {

    private PixelFactorService service;

    @Override
    public String getId() {
        return PixelFactorCredentialProvider.TYPE;
    }

    @Override
    public void init(Config.Scope ignored) {
        service = new HttpServiceClient(PluginConfig.load());
    }

    @Override
    public void postInit(KeycloakSessionFactory factory) {}

    @Override
    public CredentialProvider<?> create(KeycloakSession session) {
        return new PixelFactorCredentialProvider(session, service);
    }

    @Override
    public void close() {}
}
