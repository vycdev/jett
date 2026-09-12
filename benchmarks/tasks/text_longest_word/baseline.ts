function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextError = "empty" | "malformed" | "range";
export type TextOutcome = { readonly kind: "accepted"; readonly value: string } | { readonly kind: "rejected"; readonly error: TextError };

export function solve(text: string): TextOutcome {
    let best: string = "";
    let word: string = "";
    let pos: number = 0;
    while ((pos <= length(text))) {
        let ch: string = at(text, pos);
        if (((ch == " ") || (pos == length(text)))) {
            if ((length(word) > length(best))) {
                best = word;
            }
            word = "";
        }
        else {
            word = (word + ch);
        }
        pos = (pos + 1);
    }
    if ((best == "")) {
        return {kind: "rejected", error: "empty"};
    }
    return {kind: "accepted", value: best};
}
