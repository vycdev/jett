function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }
function new_words(): string[] { return []; }
function push(values: string[], value: string): string[] { return [...values, value]; }

export type TextError = "empty" | "malformed" | "range";
export type TextOutcome = { readonly kind: "accepted"; readonly value: string[] } | { readonly kind: "rejected"; readonly error: TextError };

export function quoted_end(text: string, start: number): number {
    let pos: number = (start + 1);
    while ((pos < length(text))) {
        if ((at(text, pos) == "\"")) {
            if ((at(text, (pos + 1)) == "\"")) {
                pos = (pos + 2);
            }
            else {
                return (pos + 1);
            }
        }
        else {
            pos = (pos + 1);
        }
    }
    return (-1);
}

export function field_end(text: string, start: number): number {
    if ((at(text, start) == "\"")) {
        return quoted_end(text, start);
    }
    let pos: number = start;
    while (((pos < length(text)) && (at(text, pos) != ","))) {
        if ((at(text, pos) == "\"")) {
            return (-1);
        }
        pos = (pos + 1);
    }
    return pos;
}

export function decoded_field(text: string, start: number, end: number): string {
    if ((at(text, start) != "\"")) {
        return part(text, start, end);
    }
    let out: string = "";
    let pos: number = (start + 1);
    while ((pos < (end - 1))) {
        let ch: string = at(text, pos);
        out = (out + ch);
        if ((ch == "\"")) {
            pos = (pos + 1);
        }
        pos = (pos + 1);
    }
    return out;
}

export function solve(text: string): TextOutcome {
    let pos: number = 0;
    let fields: string[] = new_words();
    while ((pos <= length(text))) {
        let end: number = field_end(text, pos);
        if ((end < 0)) {
            return {kind: "rejected", error: "malformed"};
        }
        fields = push(fields, decoded_field(text, pos, end));
        if ((end == length(text))) {
            return {kind: "accepted", value: fields};
        }
        if ((at(text, end) != ",")) {
            return {kind: "rejected", error: "malformed"};
        }
        pos = (end + 1);
    }
    return {kind: "rejected", error: "malformed"};
}
