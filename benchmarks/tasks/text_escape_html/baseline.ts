function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function escape_char(ch: string): string {
    if ((ch == "&")) {
        return "&amp;";
    }
    if ((ch == "<")) {
        return "&lt;";
    }
    if ((ch == ">")) {
        return "&gt;";
    }
    if ((ch == "\"")) {
        return "&quot;";
    }
    if ((ch == "'")) {
        return "&#39;";
    }
    return ch;
}

export function solve(text: string): string {
    let out: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        out = (out + escape_char(at(text, pos)));
        pos = (pos + 1);
    }
    return out;
}
