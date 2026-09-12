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

export function append_word(out: string, word: string, used: number, width: number): string {
    if ((out == "")) {
        return word;
    }
    if ((((used + 1) + length(word)) <= width)) {
        return ((out + " ") + word);
    }
    return ((out + "\n") + word);
}

export function solve(text: string, width: number): string {
    let clean: string = compact(text);
    let out: string = "";
    let used: number = 0;
    let start: number = 0;
    let pos: number = 0;
    while ((pos <= length(clean))) {
        if (((at(clean, pos) == " ") || (pos == length(clean)))) {
            let word: string = part(clean, start, pos);
            out = append_word(out, word, used, width);
            if (((used == 0) || (((used + 1) + length(word)) > width))) {
                used = length(word);
            }
            else {
                used = ((used + 1) + length(word));
            }
            start = (pos + 1);
        }
        pos = (pos + 1);
    }
    return out;
}
