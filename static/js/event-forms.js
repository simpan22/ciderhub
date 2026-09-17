// Auto-advance behavior for the "Log X" event forms on a batch's detail
// page: on a successful submit, the form is cleared but its date is
// preserved (not reset to today), the card collapses, and the next card
// in the cidermaking sequence opens with that date carried forward.
// Measurement is the one exception — it stays open, since logging
// several readings in a row is the common case.
(function () {
    const SEQUENCE = ["juicing", "additive", "measurement", "racking", "bottling"];
    const STAYS_OPEN = new Set(["measurement"]);

    function seasonYear() {
        const el = document.querySelector("[data-season-year]");
        return el ? parseInt(el.dataset.seasonYear, 10) : null;
    }

    // Juicing/picking forms carry month+day (year is implied by the
    // batch's season); every other event type carries a full
    // occurred_at date. Returns a shape-tagged value so setDate can
    // convert between the two when the next form uses the other shape.
    function readDate(form) {
        const month = form.querySelector('[name="month"]');
        const day = form.querySelector('[name="day"]');
        if (month && day) {
            return { month: parseInt(month.value, 10), day: parseInt(day.value, 10) };
        }
        const occurredAt = form.querySelector('[name="occurred_at"]');
        return occurredAt && occurredAt.value ? { iso: occurredAt.value } : null;
    }

    function setDate(form, date) {
        if (!date) {
            return;
        }
        const month = form.querySelector('[name="month"]');
        const day = form.querySelector('[name="day"]');
        if (month && day) {
            if (date.month != null) {
                month.value = date.month;
                day.value = date.day;
            } else if (date.iso) {
                const parts = date.iso.split("-");
                month.value = parseInt(parts[1], 10);
                day.value = parseInt(parts[2], 10);
            }
            return;
        }

        const occurredAt = form.querySelector('[name="occurred_at"]');
        if (!occurredAt) {
            return;
        }
        if (date.iso) {
            occurredAt.value = date.iso;
        } else if (date.month != null) {
            const year = seasonYear();
            if (year) {
                const mm = String(date.month).padStart(2, "0");
                const dd = String(date.day).padStart(2, "0");
                occurredAt.value = year + "-" + mm + "-" + dd;
            }
        }
    }

    window.ciderhubAfterEventSubmit = function (formEl, kind) {
        const date = readDate(formEl);
        formEl.reset();
        setDate(formEl, date);

        if (STAYS_OPEN.has(kind)) {
            return;
        }

        const details = formEl.closest("details");
        if (details) {
            details.open = false;
        }

        const idx = SEQUENCE.indexOf(kind);
        if (idx === -1 || idx === SEQUENCE.length - 1) {
            return;
        }

        const nextDetails = document.getElementById("log-" + SEQUENCE[idx + 1]);
        if (!nextDetails) {
            return;
        }
        const nextForm = nextDetails.querySelector("form");
        if (nextForm) {
            setDate(nextForm, date);
        }
        nextDetails.open = true;
    };
})();
