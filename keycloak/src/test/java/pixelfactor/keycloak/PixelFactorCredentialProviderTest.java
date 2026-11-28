package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.doThrow;
import static org.mockito.Mockito.inOrder;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.never;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.keycloak.models.KeycloakSession;
import org.keycloak.models.RealmModel;
import org.keycloak.models.SubjectCredentialManager;
import org.keycloak.models.UserModel;
import org.mockito.InOrder;

class PixelFactorCredentialProviderTest {

    private final PixelFactorService service = mock(PixelFactorService.class);
    private final UserModel user = mock(UserModel.class);
    private final SubjectCredentialManager credentials = mock(SubjectCredentialManager.class);
    private final RealmModel realm = mock(RealmModel.class);
    private final PixelFactorCredentialProvider provider =
            new PixelFactorCredentialProvider(mock(KeycloakSession.class), service);

    @BeforeEach
    void wire() {
        when(user.getId()).thenReturn("uuid-1");
        when(user.credentialManager()).thenReturn(credentials);
        when(credentials.removeStoredCredentialById("cred-1")).thenReturn(true);
    }

    @Test
    void deletesTheServiceRecordFirstThenTheMarker() throws Exception {
        assertTrue(provider.deleteCredential(realm, user, "cred-1"));
        InOrder order = inOrder(service, credentials);
        order.verify(service).delete("uuid-1");
        order.verify(credentials).removeStoredCredentialById("cred-1");
    }

    @Test
    void keepsTheMarkerWhenTheServiceCannotDelete() throws Exception {
        doThrow(new PixelFactorService.ServiceException("down")).when(service).delete("uuid-1");
        assertFalse(provider.deleteCredential(realm, user, "cred-1"));
        verify(credentials, never()).removeStoredCredentialById(anyString());
    }
}
