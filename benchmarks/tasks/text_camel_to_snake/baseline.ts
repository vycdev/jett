function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function boundary(text: string, pos: number): boolean {
    if ((pos == 0)) {
        return false;
    }
    let ch: string = at(text, pos);
    if ((!contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch))) {
        return false;
    }
    let prev: string = at(text, (pos - 1));
    if (contains("abcdefghijklmnopqrstuvwxyz0123456789", prev)) {
        return true;
    }
    return (contains("abcdefghijklmnopqrstuvwxyz", at(text, (pos + 1))) && ((pos + 1) < length(text)));
}

export function solve(text: string): string {
    let out: string = "";
    let pos: number = 0;
    while ((pos < length(text))) {
        if (boundary(text, pos)) {
            out = (out + "_");
        }
        out = (out + lower(at(text, pos)));
        pos = (pos + 1);
    }
    return out;
}
