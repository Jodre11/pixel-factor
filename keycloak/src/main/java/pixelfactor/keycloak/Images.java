package pixelfactor.keycloak;

import java.util.List;
import java.util.Optional;
import java.util.regex.Pattern;

/** The image files shipped in the jar's theme resources, which must match the service's {@code PF_IMAGES}. */
final class Images {

    private static final Pattern ID = Pattern.compile("[A-Za-z0-9_-]{1,64}");
    private static final List<String> EXTENSIONS = List.of("svg", "png", "jpg", "webp");
    private static final String DIR = "theme-resources/resources/img/";

    private Images() {}

    /**
     * The file name, under the theme's {@code img/} resources, for an image id, or empty if the jar has no such image.
     * A missing image is a deployment error: Keycloak and the service disagree.
     */
    static Optional<String> fileFor(String imageId) {
        if (imageId == null || !ID.matcher(imageId).matches()) {
            return Optional.empty();
        }
        ClassLoader loader = Images.class.getClassLoader();
        for (String ext : EXTENSIONS) {
            String name = imageId + "." + ext;
            if (loader.getResource(DIR + name) != null) {
                return Optional.of(name);
            }
        }
        return Optional.empty();
    }
}
