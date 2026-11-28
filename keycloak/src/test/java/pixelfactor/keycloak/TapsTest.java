package pixelfactor.keycloak;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.List;
import org.junit.jupiter.api.Test;

class TapsTest {

    @Test
    void roundTripsAndStripsFiller() {
        List<double[]> taps = List.of(new double[] {0.25, 0.5}, new double[] {0.987654, 0.000001});
        String field = Taps.format(taps, 8);
        List<double[]> parsed = Taps.parse(field, 8);
        assertEquals(2, parsed.size());
        assertArrayEquals(taps.get(0), parsed.get(0), 1e-9);
        assertArrayEquals(taps.get(1), parsed.get(1), 1e-9);
    }

    @Test
    void theFieldIsTheSameLengthWhateverTheCount() {
        int empty = Taps.format(List.of(), 8).length();
        int one = Taps.format(List.of(new double[] {0.1, 0.2}), 8).length();
        int full = Taps.format(java.util.Collections.nCopies(8, new double[] {0.9, 0.7}), 8)
                .length();
        assertEquals(empty, one);
        assertEquals(empty, full);
    }

    @Test
    void anythingMalformedIsAnEmptySequence() {
        String good = Taps.format(List.of(new double[] {0.1, 0.2}), 4);
        assertTrue(Taps.parse(null, 4).isEmpty());
        assertTrue(Taps.parse("", 4).isEmpty());
        assertTrue(Taps.parse(good, 5).isEmpty(), "wrong number of slots");
        assertTrue(Taps.parse(good.replace("00.100000", "0.1"), 4).isEmpty(), "not fixed width");
        assertTrue(Taps.parse(good.replace("00.100000", "-0.10000"), 4).isEmpty(), "sign");
        assertTrue(Taps.parse(good.replace("00.100000", "1e-100000"), 4).isEmpty(), "exponent");
        assertTrue(Taps.parse(good + ";", 4).isEmpty(), "trailing separator");
        String fillerFirst = Taps.FILLER + ";00.100000,00.200000;" + Taps.FILLER + ";" + Taps.FILLER;
        assertTrue(Taps.parse(fillerFirst, 4).isEmpty(), "a tap after filler");
        assertTrue(Taps.parse(good, 0).isEmpty());
    }

    @Test
    void allFillerIsAnEmptySequence() {
        assertTrue(Taps.parse(Taps.format(List.of(), 3), 3).isEmpty());
    }
}
