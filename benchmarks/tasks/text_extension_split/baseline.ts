function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextPair = { readonly first: string; readonly second: string };

export function solve(text: string): TextPair {
    let dot: number = (-1);
    let pos: number = 1;
    while ((pos < length(text))) {
        if ((at(text, pos) == ".")) {
            dot = pos;
        }
        pos = (pos + 1);
    }
    if ((dot < 0)) {
        return {first: text, second: ""};
    }
    return {first: part(text, 0, dot), second: part(text, (dot + 1), length(text))};
}
