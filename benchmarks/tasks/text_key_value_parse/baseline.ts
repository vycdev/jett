function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }

export type TextPair = { readonly first: string; readonly second: string };

export function trim_space(text: string): string {
    let start: number = 0;
    let end: number = length(text);
    while (((start < end) && (at(text, start) == " "))) {
        start = (start + 1);
    }
    while (((end > start) && (at(text, (end - 1)) == " "))) {
        end = (end - 1);
    }
    return part(text, start, end);
}

export function solve(text: string): TextPair {
    let pos: number = 0;
    while ((at(text, pos) != "=")) {
        pos = (pos + 1);
    }
    return {first: trim_space(part(text, 0, pos)), second: trim_space(part(text, (pos + 1), length(text)))};
}
