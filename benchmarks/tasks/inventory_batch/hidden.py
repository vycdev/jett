from solution import Accepted, Rejected, Sale, Stock, apply_inventory


assert apply_inventory([]) == Accepted({})
assert apply_inventory([Stock("apple", 5), Sale("apple", 2), Stock("banana", 4)]) == Accepted({"apple": 3, "banana": 4})
assert apply_inventory([Stock("apple", 2), Sale("apple", 2)]) == Accepted({"apple": 0})
assert apply_inventory([Sale("apple", 1)]) == Rejected(0, "apple")
assert apply_inventory([Stock("apple", 2), Sale("apple", 3), Stock("banana", 9)]) == Rejected(1, "apple")
assert apply_inventory([Stock("bad", 0)]) == Rejected(0, "bad")
