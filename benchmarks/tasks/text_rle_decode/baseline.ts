function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextError = "empty" | "malformed" | "range";
export type TextOutcome = { readonly kind: "accepted"; readonly value: string } | { readonly kind: "rejected"; readonly error: TextError };

export function digit(ch: string): number {
    let pos: number = 0;
    while ((pos < 10)) {
        if ((at("0123456789", pos) == ch)) {
            return pos;
        }
        pos = (pos + 1);
    }
    return (-1);
}

export function uint_value(text: string, bound: number): number {
    if ((length(text) == 0)) {
        return (-1);
    }
    let value: number = 0;
    let pos: number = 0;
    while ((pos < length(text))) {
        let num: number = digit(at(text, pos));
        if ((num < 0)) {
            return (-1);
        }
        value = ((value * 10) + num);
        if ((value > bound)) {
            return (-1);
        }
        pos = (pos + 1);
    }
    return value;
}

export function repeat_letter(ch: string, count: number): string {
    let out: string = "";
    let pos: number = 0;
    while ((pos < count)) {
        out = (out + ch);
        pos = (pos + 1);
    }
    return out;
}

export function solve(text: string): TextOutcome {
    let out: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        let start: number = pos;
        while (((pos < length(text)) && (digit(at(text, pos)) >= 0))) {
            pos = (pos + 1);
        }
        let count: number = uint_value(part(text, start, pos), 100);
        if (((count <= 0) || (at(text, start) == "0"))) {
            return {kind: "rejected", error: "malformed"};
        }
        let ch: string = at(text, pos);
        if (((pos == length(text)) || (!contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch)))) {
            return {kind: "rejected", error: "malformed"};
        }
        if (((length(out) + count) > 100)) {
            return {kind: "rejected", error: "range"};
        }
        out = (out + repeat_letter(ch, count));
        pos = (pos + 1);
    }
    return {kind: "accepted", value: out};
}
