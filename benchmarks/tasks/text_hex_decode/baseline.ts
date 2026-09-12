function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextError = "empty" | "malformed" | "range";
export type TextOutcome = { readonly kind: "accepted"; readonly value: string } | { readonly kind: "rejected"; readonly error: TextError };

export function hex_digit(ch: string): number {
    let pos: number = 0;
    while ((pos < 16)) {
        if ((at("0123456789abcdef", pos) == lower(ch))) {
            return pos;
        }
        pos = (pos + 1);
    }
    return (-1);
}

export function ascii_print(code: number): string {
    return at(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~", (code - 32));
}

export function solve(text: string): TextOutcome {
    if (((length(text) % 2) != 0)) {
        return {kind: "rejected", error: "malformed"};
    }
    let out: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        let high: number = hex_digit(at(text, pos));
        let low: number = hex_digit(at(text, (pos + 1)));
        if (((high < 0) || (low < 0))) {
            return {kind: "rejected", error: "malformed"};
        }
        let code: number = ((high * 16) + low);
        if (((code < 32) || (code > 126))) {
            return {kind: "rejected", error: "range"};
        }
        out = (out + ascii_print(code));
        pos = (pos + 2);
    }
    return {kind: "accepted", value: out};
}
