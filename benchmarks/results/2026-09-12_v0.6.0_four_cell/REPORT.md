# 100-problem four-cell benchmark

Model: gpt-5.6-luna; reasoning: medium; backend: Codex subscription.

| Language | Context | Budget | Pass | Input tokens | Cached input | Output tokens | Reasoning tokens | Latency (s) | Code chars | Code bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| go | zero_shot | one_shot | 94/100 | 1,276,739 | 1,169,408 | 28,845 | 8,979 | 881.3 | 62,500 | 62,500 |
| go | zero_shot | compile_repair | 97/100 | 1,355,894 | 1,241,600 | 32,528 | 10,636 | 968.5 | 69,242 | 69,242 |
| go | skill_assisted | one_shot | 95/100 | 1,344,859 | 1,132,544 | 34,735 | 14,957 | 971.2 | 64,255 | 64,255 |
| go | skill_assisted | compile_repair | 98/100 | 1,414,300 | 1,192,704 | 37,531 | 16,002 | 1,041.9 | 70,064 | 70,064 |
| jett | zero_shot | one_shot | 4/100 | 1,276,560 | 1,154,048 | 121,252 | 99,688 | 2,541.5 | 82,085 | 82,085 |
| jett | zero_shot | compile_repair | 5/100 | 2,614,134 | 2,278,400 | 192,716 | 150,639 | 4,186.5 | 161,507 | 161,507 |
| jett | skill_assisted | one_shot | 33/100 | 1,476,343 | 1,187,840 | 96,501 | 71,461 | 2,102.7 | 103,174 | 103,174 |
| jett | skill_assisted | compile_repair | 66/100 | 2,493,187 | 1,987,840 | 145,918 | 99,554 | 3,243.3 | 190,698 | 190,698 |
| python | zero_shot | one_shot | 76/100 | 1,277,792 | 1,138,944 | 25,651 | 9,814 | 826.1 | 60,858 | 60,858 |
| python | zero_shot | compile_repair | 96/100 | 1,591,426 | 1,424,640 | 32,709 | 12,392 | 1,039.3 | 77,934 | 77,934 |
| python | skill_assisted | one_shot | 92/100 | 1,348,467 | 1,157,120 | 30,249 | 13,755 | 910.3 | 64,213 | 64,213 |
| python | skill_assisted | compile_repair | 97/100 | 1,458,746 | 1,253,376 | 34,705 | 16,270 | 1,019.0 | 71,840 | 71,840 |
| rust | zero_shot | one_shot | 90/100 | 1,277,919 | 1,141,760 | 32,454 | 11,991 | 934.9 | 78,187 | 78,187 |
| rust | zero_shot | compile_repair | 97/100 | 1,408,533 | 1,262,080 | 36,576 | 13,532 | 1,045.7 | 88,094 | 88,094 |
| rust | skill_assisted | one_shot | 94/100 | 1,357,373 | 1,150,976 | 43,846 | 22,796 | 1,141.7 | 82,822 | 82,822 |
| rust | skill_assisted | compile_repair | 95/100 | 1,440,855 | 1,223,168 | 47,954 | 25,134 | 1,236.5 | 89,515 | 89,515 |
| typescript | zero_shot | one_shot | 94/100 | 1,277,141 | 1,160,192 | 31,445 | 10,813 | 959.7 | 76,863 | 76,863 |
| typescript | zero_shot | compile_repair | 97/100 | 1,356,584 | 1,232,384 | 35,324 | 12,574 | 1,049.2 | 84,647 | 84,647 |
| typescript | skill_assisted | one_shot | 94/100 | 1,349,141 | 1,178,624 | 39,763 | 18,254 | 1,077.7 | 82,324 | 82,324 |
| typescript | skill_assisted | compile_repair | 97/100 | 1,432,745 | 1,250,816 | 44,422 | 20,732 | 1,182.5 | 90,403 | 90,403 |

Repair budgets include initial and repair usage; passing initial responses receive no second prompt.
Code counts include every generated candidate, including failures. Reasoning tokens are included in output tokens.
Cached input is a subset of input tokens. Latency sums model-call durations, not parallel campaign wall time.

Each task/context has one observation. The model alias can change; these are calibration results.
The task set is used for skill development; repeated evaluation is not an untouched held-out estimate.
No API-billed requests were used; subscription allowance was consumed.
