function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


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

export function alpha(ch: string): boolean {
    return (contains("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ", ch) && (length(ch) == 1));
}

export function alnum(ch: string): boolean {
    return (alpha(ch) || (digit(ch) >= 0));
}

export function solve(text: string): boolean {
    let clean: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if (alnum(ch)) {
            clean = (clean + lower(ch));
        }
        pos = (pos + 1);
    }
    pos = 0;
    while ((pos < length(clean))) {
        if ((at(clean, pos) != at(clean, ((length(clean) - 1) - pos)))) {
            return false;
        }
        pos = (pos + 1);
    }
    return true;
}
