package pixelfactor.keycloak;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.regex.Pattern;

/**
 * The fixed-length form field the tap page submits.
 *
 * <p>The field always holds {@code maxTaps} pairs separated by {@code ;}, each pair {@code x,y}, each number exactly
 * nine characters ({@code 00.123456}). Unused pairs at the end are {@link #FILLER}. The request is therefore the same
 * size whatever the sequence, so its length does not reveal the tap count.
 *
 * <p>Parsing never rejects anything on its own account: input that does not fit the format becomes an empty sequence,
 * which the service answers with no match. The plugin thereby adds no signal of its own.
 */
public final class Taps {

    /** The value of an unused pair. Outside any image, so it can never be a real tap. */
    public static final String FILLER = "99.999999,99.999999";

    private static final Pattern NUMBER = Pattern.compile("\\d{2}\\.\\d{6}");

    private Taps() {}

    /** The taps in the field, or an empty list if it does not have exactly {@code maxTaps} well-formed pairs. */
    public static List<double[]> parse(String field, int maxTaps) {
        if (field == null || maxTaps < 1) {
            return List.of();
        }
        String[] pairs = field.split(";", -1);
        if (pairs.length != maxTaps) {
            return List.of();
        }
        List<double[]> taps = new ArrayList<>();
        boolean filling = false;
        for (String pair : pairs) {
            if (FILLER.equals(pair)) {
                filling = true;
                continue;
            }
            if (filling) {
                // A real tap after filler is not something the page ever sends.
                return List.of();
            }
            String[] xy = pair.split(",", -1);
            if (xy.length != 2
                    || !NUMBER.matcher(xy[0]).matches()
                    || !NUMBER.matcher(xy[1]).matches()) {
                return List.of();
            }
            taps.add(new double[] {Double.parseDouble(xy[0]), Double.parseDouble(xy[1])});
        }
        return taps;
    }

    /** Format taps the way the page does. For tests and documentation of the format. */
    public static String format(List<double[]> taps, int maxTaps) {
        List<String> pairs = new ArrayList<>();
        for (double[] tap : taps) {
            pairs.add(String.format(Locale.ROOT, "%09.6f,%09.6f", tap[0], tap[1]));
        }
        while (pairs.size() < maxTaps) {
            pairs.add(FILLER);
        }
        return String.join(";", pairs);
    }
}
