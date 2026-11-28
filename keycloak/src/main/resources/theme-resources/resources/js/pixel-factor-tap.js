// The tap page. Records taps on the image in units of image width and submits them as a fixed-length field.
// Taps come from a pointer, or from the keyboard: arrow keys move a cursor over the image and Enter or Space
// places a tap there. No libraries. See docs/keycloak.md, "The tap page".
(function () {
    "use strict";

    var stage = document.getElementById("pf-stage");
    var image = document.getElementById("pf-image");
    var cursor = document.getElementById("pf-cursor");
    var live = document.getElementById("pf-live");
    var field = document.getElementById("pf-taps");
    var form = document.getElementById("pf-form");
    if (!stage || !image || !cursor || !live || !field || !form) {
        return;
    }
    var maxTaps = parseInt(stage.getAttribute("data-max-taps"), 10);
    var filler = stage.getAttribute("data-filler");
    var taps = [];
    var dots = [];
    // The keyboard cursor, in units of image width. Starts in the middle of the image.
    var cx = 0.5;
    var cy = -1;

    // Generic messages only: never the position, and nothing the numbered dots do not already show.
    function announce(key) {
        live.textContent = "";
        window.setTimeout(function () {
            live.textContent = stage.getAttribute("data-msg-" + key);
        }, 50);
    }

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

    function aspect() {
        var box = image.getBoundingClientRect();
        return box.width > 0 ? box.height / box.width : 0;
    }

    function draw() {
        while (dots.length) {
            stage.removeChild(dots.pop());
        }
        var width = image.getBoundingClientRect().width;
        for (var i = 0; i < taps.length; i++) {
            var dot = document.createElement("span");
            dot.className = "pf-dot";
            dot.setAttribute("aria-hidden", "true");
            dot.textContent = String(i + 1);
            dot.style.left = (taps[i][0] * width) + "px";
            dot.style.top = (taps[i][1] * width) + "px";
            stage.appendChild(dot);
            dots.push(dot);
        }
        if (cy < 0) {
            cy = aspect() / 2;
        }
        cursor.style.left = (cx * width) + "px";
        cursor.style.top = (cy * width) + "px";
    }

    function add(x, y) {
        if (taps.length >= maxTaps) {
            announce("full");
            return;
        }
        taps.push([x, y]);
        draw();
        announce("added");
    }

    image.addEventListener("click", function (event) {
        var box = image.getBoundingClientRect();
        var px = event.clientX - box.left;
        var py = event.clientY - box.top;
        // Clicks outside the drawn image are ignored.
        if (box.width <= 0 || px < 0 || py < 0 || px > box.width || py > box.height) {
            return;
        }
        add(px / box.width, py / box.width);
    });

    stage.addEventListener("keydown", function (event) {
        var step = event.shiftKey ? 0.05 : 0.01;
        var limit = aspect();
        switch (event.key) {
            case "ArrowLeft": cx = Math.max(0, cx - step); break;
            case "ArrowRight": cx = Math.min(1, cx + step); break;
            case "ArrowUp": cy = Math.max(0, cy - step); break;
            case "ArrowDown": cy = Math.min(limit, cy + step); break;
            case "Enter":
            case " ":
                add(cx, cy);
                break;
            case "Backspace":
                undo();
                break;
            default:
                return;
        }
        event.preventDefault();
        draw();
    });

    function undo() {
        if (taps.length) {
            taps.pop();
            draw();
            announce("removed");
        }
    }

    document.getElementById("pf-undo").addEventListener("click", undo);

    document.getElementById("pf-clear").addEventListener("click", function () {
        taps = [];
        draw();
        announce("cleared");
    });

    window.addEventListener("resize", draw);
    if (image.complete) {
        draw();
    } else {
        image.addEventListener("load", draw);
    }

    form.addEventListener("submit", function () {
        field.value = encode();
    });
}());
