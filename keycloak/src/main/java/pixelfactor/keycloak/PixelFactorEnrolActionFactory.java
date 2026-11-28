package pixelfactor.keycloak;

import org.keycloak.Config;
import org.keycloak.authentication.RequiredActionFactory;
import org.keycloak.authentication.RequiredActionProvider;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.KeycloakSessionFactory;

/** Registers enrolment, required action id {@value #ID}. */
public final class PixelFactorEnrolActionFactory implements RequiredActionFactory {

    /** The required action id. */
    public static final String ID = "pixel-factor-enrol";

    private PixelFactorService service;

    @Override
    public String getId() {
        return ID;
    }

    @Override
    public String getDisplayText() {
        return "Set up tap sequence";
    }

    @Override
    public void init(Config.Scope ignored) {
        service = new HttpServiceClient(PluginConfig.load());
    }

    @Override
    public void postInit(KeycloakSessionFactory factory) {}

    @Override
    public RequiredActionProvider create(KeycloakSession session) {
        return new PixelFactorEnrolAction(service);
    }

    @Override
    public void close() {}
}
