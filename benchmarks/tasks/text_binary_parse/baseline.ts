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

export function solve(text: string): TextOutcome {
    if ((length(text) == 0)) {
        return {kind: "rejected", error: "empty"};
    }
    if ((length(text) > 16)) {
        return {kind: "rejected", error: "range"};
    }
    let value: number = 0;
    let pos: number = 0;
    while ((pos < length(text))) {
        let num: number = digit(at(text, pos));
        if (((num < 0) || (num > 1))) {
            return {kind: "rejected", error: "malformed"};
        }
        value = ((value * 2) + num);
        pos = (pos + 1);
    }
    return {kind: "accepted", value: value};
}
