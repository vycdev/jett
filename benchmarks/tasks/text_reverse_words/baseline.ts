function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function compact(text: string): string {
    let out: string = "";
    let pending: boolean = false;
    let pos: number = 0;
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if ((ch == " ")) {
            pending = (length(out) > 0);
        }
        else {
            if (pending) {
                out = (out + " ");
            }
            out = (out + ch);
            pending = false;
        }
        pos = (pos + 1);
    }
    return out;
}

export function solve(text: string): string {
    let clean: string = compact(text);
    let out: string = "";
    let end: number = length(clean);
    let pos: number = (length(clean) - 1);
    while ((pos >= 0)) {
        if ((at(clean, pos) == " ")) {
            out = ((out + part(clean, (pos + 1), end)) + " ");
            end = pos;
        }
        pos = (pos - 1);
    }
    return (out + part(clean, 0, end));
}
