// The tap page. Records taps on the image in units of image width and submits them as a fixed-length field.
// No libraries. See docs/keycloak.md, "The tap page".
(function () {
    "use strict";

    var stage = document.getElementById("pf-stage");
    var image = document.getElementById("pf-image");
    var field = document.getElementById("pf-taps");
    var form = document.getElementById("pf-form");
    if (!stage || !image || !field || !form) {
        return;
    }
    var maxTaps = parseInt(stage.getAttribute("data-max-taps"), 10);
    var filler = stage.getAttribute("data-filler");
    var taps = [];
    var dots = [];

    // Two integer digits and six decimals, so every pair has the same width whatever its value.
    function fixed(value) {
        var text = value.toFixed(6);
        while (text.length < 9) {
            text = "0" + text;
        }
        return text;
    }

    function encode() {
        var pairs = [];
        for (var i = 0; i < maxTaps; i++) {
            pairs.push(i < taps.length ? fixed(taps[i][0]) + "," + fixed(taps[i][1]) : filler);
        }
        return pairs.join(";");
    }

    function draw() {
        while (dots.length) {
            stage.removeChild(dots.pop());
        }
        var box = image.getBoundingClientRect();
        for (var i = 0; i < taps.length; i++) {
            var dot = document.createElement("span");
            dot.className = "pf-dot";
            dot.textContent = String(i + 1);
            dot.style.left = (taps[i][0] * box.width) + "px";
            dot.style.top = (taps[i][1] * box.width) + "px";
            stage.appendChild(dot);
            dots.push(dot);
        }
    }

    image.addEventListener("click", function (event) {
        if (taps.length >= maxTaps) {
            return;
        }
        var box = image.getBoundingClientRect();
        var px = event.clientX - box.left;
        var py = event.clientY - box.top;
        // Clicks outside the drawn image are ignored.
        if (box.width <= 0 || px < 0 || py < 0 || px > box.width || py > box.height) {
            return;
        }
        taps.push([px / box.width, py / box.width]);
        draw();
    });

    document.getElementById("pf-undo").addEventListener("click", function () {
        taps.pop();
        draw();
    });

    document.getElementById("pf-clear").addEventListener("click", function () {
        taps = [];
        draw();
    });

    window.addEventListener("resize", draw);

    form.addEventListener("submit", function () {
        field.value = encode();
    });
}());
