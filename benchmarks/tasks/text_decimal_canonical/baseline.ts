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

export function signed_digits(negative: boolean, digits: string): TextOutcome {
    if ((digits == "")) {
        return {kind: "accepted", value: "0"};
    }
    if (negative) {
        return {kind: "accepted", value: ("-" + digits)};
    }
    return {kind: "accepted", value: digits};
}

export function solve(text: string): TextOutcome {
    if ((length(text) == 0)) {
        return {kind: "rejected", error: "empty"};
    }
    let pos: number = 0;
    let negative: boolean = (at(text, 0) == "-");
    if ((negative || (at(text, 0) == "+"))) {
        pos = 1;
    }
    if ((pos == length(text))) {
        return {kind: "rejected", error: "malformed"};
    }
    let out: string = "";
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if ((digit(ch) < 0)) {
            return {kind: "rejected", error: "malformed"};
        }
        if (((out != "") || (ch != "0"))) {
            out = (out + ch);
        }
        pos = (pos + 1);
    }
    return signed_digits(negative, out);
}
