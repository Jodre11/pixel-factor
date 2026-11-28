package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class HttpServiceClientTest {

    private record Canned(int status, String body) {}

    private HttpServer server;
    private final AtomicReference<Canned> answer = new AtomicReference<>();
    private final AtomicReference<String> seenAuth = new AtomicReference<>();
    private final AtomicReference<String> seenPath = new AtomicReference<>();
    private final AtomicReference<String> seenBody = new AtomicReference<>();
    private HttpServiceClient client;

    @BeforeEach
    void start() throws IOException {
        server = HttpServer.create(new InetSocketAddress(InetAddress.getLoopbackAddress(), 0), 0);
        server.createContext("/", exchange -> {
            seenAuth.set(exchange.getRequestHeaders().getFirst("Authorization"));
            seenPath.set(exchange.getRequestURI().getRawPath());
            seenBody.set(new String(exchange.getRequestBody().readAllBytes(), StandardCharsets.UTF_8));
            Canned canned = answer.get();
            byte[] bytes = canned.body.getBytes(StandardCharsets.UTF_8);
            exchange.sendResponseHeaders(canned.status, bytes.length == 0 ? -1 : bytes.length);
            try (OutputStream out = exchange.getResponseBody()) {
                out.write(bytes);
            }
        });
        server.start();
        URI base = URI.create("http://127.0.0.1:" + server.getAddress().getPort() + "/");
        client = new HttpServiceClient(new PluginConfig(base, "tok"));
    }

    @AfterEach
    void stop() {
        server.stop(0);
    }

    private void reply(int status, String body) {
        answer.set(new Canned(status, body));
    }

    @Test
    void challenge() throws Exception {
        reply(200, "{\"image_id\":\"demo\",\"max_taps\":8}");
        assertEquals(new PixelFactorService.Challenge("demo", 8), client.challenge("u1"));
        assertEquals("Bearer tok", seenAuth.get());
        assertEquals("/v1/challenge", seenPath.get());
        assertEquals("{\"user_id\":\"u1\"}", seenBody.get());
    }

    @Test
    void verifySendsTapsAsPairs() throws Exception {
        reply(200, "{\"match\":true,\"locked\":false}");
        PixelFactorService.Verification v = client.verify("u1", List.of(new double[] {0.25, 0.5}));
        assertTrue(v.match());
        assertFalse(v.locked());
        assertTrue(seenBody.get().contains("\"taps\":[[0.25,0.5]]"), seenBody.get());
    }

    @Test
    void stagingAcceptsRejectsAndTreatsUnknownImageAsAnError() throws Exception {
        reply(201, "{\"warnings\":[\"taps 2 and 3 are close together\"]}");
        PixelFactorService.Staged staged = client.stageEnrolment("u1", "demo", List.of());
        assertTrue(staged.accepted());
        assertEquals(List.of("taps 2 and 3 are close together"), staged.warnings());

        reply(422, "{\"error\":\"too few taps\"}");
        staged = client.stageEnrolment("u1", "demo", List.of());
        assertFalse(staged.accepted());
        assertEquals("too few taps", staged.reason());

        reply(422, "{\"error\":\"unknown image\"}");
        assertThrows(PixelFactorService.ServiceException.class, () -> client.stageEnrolment("u1", "x", List.of()));
    }

    @Test
    void confirmAndDelete() throws Exception {
        reply(200, "{\"confirmed\":false}");
        assertFalse(client.confirmEnrolment("u1", List.of()));
        reply(200, "{\"confirmed\":true}");
        assertTrue(client.confirmEnrolment("u1", List.of()));
        reply(204, "");
        client.delete("a b/c");
        assertEquals("/v1/users/a%20b%2Fc", seenPath.get());
    }

    @Test
    void nothingUndocumentedIsReadAsSuccess() {
        String[][] bad = {
            {"401", "{\"error\":\"unauthorised\"}"},
            {"500", "{\"error\":\"internal\"}"},
            {"200", "not json"},
            {"200", "{}"},
            {"200", "{\"match\":\"true\",\"locked\":false}"},
            {"201", "{\"match\":true,\"locked\":false}"},
            {"422", "{\"error\":\"too few taps\"}"},
        };
        for (String[] b : bad) {
            reply(Integer.parseInt(b[0]), b[1]);
            assertThrows(PixelFactorService.ServiceException.class, () -> client.verify("u", List.of()), b[1]);
        }
        reply(200, "{\"confirmed\":true}");
        assertThrows(PixelFactorService.ServiceException.class, () -> client.delete("u"));
        reply(200, "{\"image_id\":\"demo\"}");
        assertThrows(PixelFactorService.ServiceException.class, () -> client.challenge("u"));
    }

    @Test
    void anUnreachableServiceIsAnError() {
        server.stop(0);
        assertThrows(PixelFactorService.ServiceException.class, () -> client.verify("u", List.of()));
    }
}
