import { describe, it, expect } from "vitest";
import { recordPast, applyUndo, applyRedo, canUndo, canRedo, HISTORY_LIMIT } from "./history";

describe("recordPast", () => {
  it("appends the present", () => {
    expect(recordPast([1, 2], 3)).toEqual([1, 2, 3]);
  });

  it("caps the stack at the limit", () => {
    const big = Array.from({ length: HISTORY_LIMIT }, (_, i) => i);
    const out = recordPast(big, 999);
    expect(out.length).toBe(HISTORY_LIMIT);
    expect(out[out.length - 1]).toBe(999);
    expect(out[0]).toBe(1); // oldest dropped
  });
});

describe("applyUndo / applyRedo", () => {
  it("undo moves last past to present and pushes present to future", () => {
    const t = applyUndo([1, 2], 3, []);
    expect(t).toEqual({ past: [1], present: 2, future: [3] });
  });

  it("undo returns null when nothing to undo", () => {
    expect(applyUndo([], 1, [])).toBeNull();
  });

  it("redo moves first future to present and pushes present to past", () => {
    const t = applyRedo([1], 2, [3]);
    expect(t).toEqual({ past: [1, 2], present: 3, future: [] });
  });

  it("redo returns null when nothing to redo", () => {
    expect(applyRedo([], 1, [])).toBeNull();
  });

  it("round-trips: edit, undo, redo restores", () => {
    // start present=1, edit to 2 (record past)
    let past: number[] = recordPast<number>([], 1);
    let future: number[] = [];
    let present = 2;

    // undo -> present should be 1
    const u = applyUndo(past, present, future)!;
    past = u.past;
    present = u.present;
    future = u.future;
    expect(present).toBe(1);

    // redo -> present should be 2 again
    const r = applyRedo(past, present, future)!;
    expect(r.present).toBe(2);
  });
});

describe("canUndo / canRedo", () => {
  it("reflect stack emptiness", () => {
    expect(canUndo([])).toBe(false);
    expect(canUndo([1])).toBe(true);
    expect(canRedo([])).toBe(false);
    expect(canRedo([1])).toBe(true);
  });
});
