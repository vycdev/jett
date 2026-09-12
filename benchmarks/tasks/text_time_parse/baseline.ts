function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextError = "empty" | "malformed" | "range";
export type TextOutcome = { readonly kind: "accepted"; readonly value: number } | { readonly kind: "rejected"; readonly error: TextError };

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

export function solve(text: string): TextOutcome {
    if ((length(text) != 8)) {
        return {kind: "rejected", error: "malformed"};
    }
    if (((at(text, 2) != ":") || (at(text, 5) != ":"))) {
        return {kind: "rejected", error: "malformed"};
    }
    let hour: number = uint_value(part(text, 0, 2), 99);
    let minute: number = uint_value(part(text, 3, 5), 99);
    let second: number = uint_value(part(text, 6, 8), 99);
    if (((hour < 0) || (minute < 0) || (second < 0))) {
        return {kind: "rejected", error: "malformed"};
    }
    if (((hour > 23) || (minute > 59) || (second > 59))) {
        return {kind: "rejected", error: "range"};
    }
    return {kind: "accepted", value: (((hour * 3600) + (minute * 60)) + second)};
}
