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

export function octet(text: string): number {
    if (((length(text) > 1) && (at(text, 0) == "0"))) {
        return (-1);
    }
    return uint_value(text, 255);
}

export function solve(text: string): TextOutcome {
    let pos: number = 0;
    let start: number = 0;
    let count: number = 0;
    let value: number = 0;
    while ((pos <= length(text))) {
        if (((pos == length(text)) || (at(text, pos) == "."))) {
            let num: number = octet(part(text, start, pos));
            if (((num < 0) || (count == 4))) {
                return {kind: "rejected", error: "malformed"};
            }
            value = ((value * 256) + num);
            count = (count + 1);
            start = (pos + 1);
        }
        pos = (pos + 1);
    }
    if ((count != 4)) {
        return {kind: "rejected", error: "malformed"};
    }
    return {kind: "accepted", value: value};
}
