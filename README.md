# ckb-binary-patcher

Even though we developed CKB VM with the most diligence, we are all just humans, and we made mistakes sometimes, resulting in bugs of CKB VM. However due to the very nature of blockchains, we cannot just fix the bugs. Consensus changes might be introduced in bug-fixes resulting in unexpected forks of blockchains.

As a result, we are only allowed to fix bugs and introduce new behaviors when soft/hard forks are performed. Bugs thus become features in-between forks. Luckily, there usually is more than one way to achieve the same thing in bytecodes. This project provides a solution to this problem: it patches binaries directly, so we can express certain logic in a different way that is bug free. Later when soft/hard forks are performed and the bugs are fixed, the patched binary will continue to work without compatibility problems.

Note that this is not a silver bullet, it cannot solve all potential bugs, but we believe a significant number of bugs can be fixed this way. In addition, this project can also be used to do analysis on smart contracts, alerting dapp developers potential vulnerabilities, we are hoping this project can evolve into a handy tool that is used by many CKB dapp developers. Even though there might be a time we fixed all the bugs in the VM, the tool here can still be quite valuable.
