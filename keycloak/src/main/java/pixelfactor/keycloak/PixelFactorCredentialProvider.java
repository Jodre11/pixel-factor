package pixelfactor.keycloak;

import java.util.Optional;
import org.jboss.logging.Logger;
import org.keycloak.credential.CredentialModel;
import org.keycloak.credential.CredentialProvider;
import org.keycloak.credential.CredentialTypeMetadata;
import org.keycloak.credential.CredentialTypeMetadataContext;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.RealmModel;
import org.keycloak.models.UserModel;

/**
 * The marker credential. It holds no secret: the record lives in the service. It lets Keycloak tell who is enrolled
 * without a network call, shows in the account console, and deletes the service record when it is removed.
 */
public final class PixelFactorCredentialProvider implements CredentialProvider<CredentialModel> {

    /** The credential type. */
    public static final String TYPE = "pixel-factor";

    private static final Logger LOG = Logger.getLogger(PixelFactorCredentialProvider.class);

    private final KeycloakSession session;
    private final PixelFactorService service;

    /** A provider for one Keycloak session. */
    public PixelFactorCredentialProvider(KeycloakSession session, PixelFactorService service) {
        this.session = session;
        this.service = service;
    }

    @Override
    public String getType() {
        return TYPE;
    }

    /** The user's marker, if they have one. */
    public static Optional<CredentialModel> marker(UserModel user) {
        return user.credentialManager().getStoredCredentialsByTypeStream(TYPE).findFirst();
    }

    /**
     * Create the marker, or update it in place with the new image id. Never deleted and recreated: deleting it
     * deletes the record that was just made live.
     */
    public static void recordEnrolment(UserModel user, String imageId) {
        Optional<CredentialModel> existing = marker(user);
        if (existing.isPresent()) {
            CredentialModel model = existing.get();
            model.setCredentialData(credentialData(imageId));
            user.credentialManager().updateStoredCredential(model);
            return;
        }
        CredentialModel model = new CredentialModel();
        model.setType(TYPE);
        model.setUserLabel("Tap sequence");
        model.setCreatedDate(System.currentTimeMillis());
        model.setSecretData("{}");
        model.setCredentialData(credentialData(imageId));
        user.credentialManager().createStoredCredential(model);
    }

    private static String credentialData(String imageId) {
        return "{\"image_id\":" + org.keycloak.util.JsonSerialization.mapper.valueToTree(imageId) + "}";
    }

    @Override
    public CredentialModel createCredential(RealmModel realm, UserModel user, CredentialModel credential) {
        credential.setType(TYPE);
        return user.credentialManager().createStoredCredential(credential);
    }

    /**
     * Delete the record at the service first. If the service cannot be reached the deletion fails and the marker
     * stays, so Keycloak never shows a user as unenrolled while their record still exists.
     */
    @Override
    public boolean deleteCredential(RealmModel realm, UserModel user, String credentialId) {
        try {
            service.delete(user.getId());
        } catch (PixelFactorService.ServiceException e) {
            LOG.errorf("pixel-factor: could not delete the record for a user; credential kept: %s", e.getMessage());
            return false;
        }
        return user.credentialManager().removeStoredCredentialById(credentialId);
    }

    @Override
    public CredentialModel getCredentialFromModel(CredentialModel model) {
        return model;
    }

    @Override
    public CredentialTypeMetadata getCredentialTypeMetadata(CredentialTypeMetadataContext context) {
        return CredentialTypeMetadata.builder()
                .type(TYPE)
                .category(CredentialTypeMetadata.Category.BASIC_AUTHENTICATION)
                .displayName("pixelFactorDisplayName")
                .helpText("pixelFactorHelpText")
                .iconCssClass("kcAuthenticatorPasswordClass")
                .createAction(PixelFactorEnrolActionFactory.ID)
                .updateAction(PixelFactorEnrolActionFactory.ID)
                .removeable(true)
                .build(session);
    }
}
