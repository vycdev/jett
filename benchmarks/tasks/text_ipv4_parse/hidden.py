from solution import solve, TextAccepted, TextRejected, TextError

assert solve("0.0.0.0") == TextAccepted(0), 0
assert solve("255.255.255.255") == TextAccepted(4294967295), 1
assert solve("127.0.0.1") == TextAccepted(2130706433), 2
assert solve("192.168.1.1") == TextAccepted(3232235777), 3
assert solve("1.2.3.4") == TextAccepted(16909060), 4
assert solve("") == TextRejected(TextError.MALFORMED), 5
assert solve("1.2.3") == TextRejected(TextError.MALFORMED), 6
assert solve("1.2.3.4.5") == TextRejected(TextError.MALFORMED), 7
assert solve("01.2.3.4") == TextRejected(TextError.MALFORMED), 8
assert solve("256.0.0.1") == TextRejected(TextError.MALFORMED), 9
assert solve("1..2.3") == TextRejected(TextError.MALFORMED), 10
assert solve("1.2.3.-1") == TextRejected(TextError.MALFORMED), 11
assert solve("1.2.3.4 ") == TextRejected(TextError.MALFORMED), 12
