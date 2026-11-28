package pixelfactor.keycloak;

import com.fasterxml.jackson.databind.JsonNode;
import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.keycloak.util.JsonSerialization;

/**
 * {@link PixelFactorService} over HTTP with the JDK client. Any answer the API does not document is a {@link
 * ServiceException}, so no unexpected state can be read as success.
 */
public final class HttpServiceClient implements PixelFactorService {

    /** Each call gives up after this long. Fixed in code, not a setting. */
    public static final Duration TIMEOUT = Duration.ofSeconds(5);

    private static final HttpClient SHARED =
            HttpClient.newBuilder().connectTimeout(TIMEOUT).build();

    private final HttpClient http;
    private final URI base;
    private final String token;

    /** A client for the configured service, sharing one connection pool. */
    public HttpServiceClient(PluginConfig config) {
        this(SHARED, config);
    }

    HttpServiceClient(HttpClient http, PluginConfig config) {
        this.http = http;
        this.base = config.serviceUrl();
        this.token = config.apiToken();
    }

    @Override
    public Challenge challenge(String userId) throws ServiceException {
        JsonNode body = expect(post("v1/challenge", Map.of("user_id", userId)), 200);
        JsonNode image = body.get("image_id");
        JsonNode maxTaps = body.get("max_taps");
        if (image == null || !image.isTextual() || maxTaps == null || !maxTaps.canConvertToInt()) {
            throw new ServiceException("challenge: unexpected answer");
        }
        return new Challenge(image.asText(), maxTaps.asInt());
    }

    @Override
    public Staged stageEnrolment(String userId, String imageId, List<double[]> taps) throws ServiceException {
        Map<String, Object> request = new LinkedHashMap<>();
        request.put("user_id", userId);
        request.put("image_id", imageId);
        request.put("taps", taps);
        Answer answer = post("v1/enrolments", request);
        if (answer.status == 422) {
            String reason = text(answer.body, "error");
            if ("unknown image".equals(reason)) {
                // A deployment error, not something the user can fix by tapping again.
                throw new ServiceException("stage enrolment: service does not know the image");
            }
            return Staged.rejected(reason == null ? "" : reason);
        }
        JsonNode body = expect(answer, 201);
        List<String> warnings = new ArrayList<>();
        JsonNode list = body.get("warnings");
        if (list == null || !list.isArray()) {
            throw new ServiceException("stage enrolment: unexpected answer");
        }
        list.forEach(w -> warnings.add(w.asText()));
        return Staged.accepted(warnings);
    }

    @Override
    public boolean confirmEnrolment(String userId, List<double[]> taps) throws ServiceException {
        JsonNode body = expect(post("v1/enrolments/confirm", Map.of("user_id", userId, "taps", taps)), 200);
        return bool(body, "confirmed");
    }

    @Override
    public Verification verify(String userId, List<double[]> taps) throws ServiceException {
        JsonNode body = expect(post("v1/verify", Map.of("user_id", userId, "taps", taps)), 200);
        return new Verification(bool(body, "match"), bool(body, "locked"));
    }

    @Override
    public void delete(String userId) throws ServiceException {
        URI uri = base.resolve(
                "v1/users/" + URLEncoder.encode(userId, StandardCharsets.UTF_8).replace("+", "%20"));
        HttpRequest request = authorised(uri).DELETE().build();
        Answer answer = send(request);
        if (answer.status != 204) {
            throw new ServiceException("delete: status " + answer.status);
        }
    }

    private record Answer(int status, JsonNode body) {}

    private Answer post(String path, Object body) throws ServiceException {
        String json;
        try {
            json = JsonSerialization.writeValueAsString(body);
        } catch (IOException e) {
            throw new ServiceException("encode request", e);
        }
        HttpRequest request = authorised(base.resolve(path))
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(json))
                .build();
        return send(request);
    }

    private HttpRequest.Builder authorised(URI uri) {
        return HttpRequest.newBuilder(uri).timeout(TIMEOUT).header("Authorization", "Bearer " + token);
    }

    private Answer send(HttpRequest request) throws ServiceException {
        HttpResponse<String> response;
        try {
            response = http.send(request, HttpResponse.BodyHandlers.ofString());
        } catch (IOException e) {
            throw new ServiceException(
                    request.uri().getPath() + ": " + e.getClass().getSimpleName(), e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new ServiceException(request.uri().getPath() + ": interrupted", e);
        }
        JsonNode body = null;
        if (!response.body().isEmpty()) {
            try {
                body = JsonSerialization.mapper.readTree(response.body());
            } catch (IOException e) {
                body = null;
            }
        }
        return new Answer(response.statusCode(), body);
    }

    private static JsonNode expect(Answer answer, int status) throws ServiceException {
        if (answer.status != status || answer.body == null || !answer.body.isObject()) {
            throw new ServiceException("unexpected status " + answer.status);
        }
        return answer.body;
    }

    private static boolean bool(JsonNode body, String field) throws ServiceException {
        JsonNode value = body.get(field);
        if (value == null || !value.isBoolean()) {
            throw new ServiceException("missing " + field);
        }
        return value.asBoolean();
    }

    private static String text(JsonNode body, String field) {
        if (body == null) {
            return null;
        }
        JsonNode value = body.get(field);
        return value != null && value.isTextual() ? value.asText() : null;
    }
}
