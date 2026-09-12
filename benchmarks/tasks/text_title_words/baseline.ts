function length(text: string): number { return text.length; }
function at(text: string, pos: number): string { return text.charAt(pos); }
function part(text: string, start: number, end: number): string { return text.slice(start, end); }
function contains(text: string, needle: string): boolean { return text.includes(needle); }
function lower(text: string): string { return text.toLowerCase(); }
function upper(text: string): string { return text.toUpperCase(); }
function render(value: number): string { return String(value); }


export function solve(text: string): string {
    let out: string = "";
    let start: boolean = true;
    let pos: number = 0;
    while ((pos < length(text))) {
        let ch: string = at(text, pos);
        if ((ch == " ")) {
            out = (out + ch);
            start = true;
        }
        else {
            if (start) {
                out = (out + upper(ch));
            }
            else {
                out = (out + lower(ch));
            }
            start = false;
        }
        pos = (pos + 1);
    }
    return out;
}
