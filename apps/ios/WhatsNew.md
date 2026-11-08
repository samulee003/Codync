<!--
App Store "What's New" for the iPhone app. Write it in the change that bumps the version:
a `## <version>` section with one `### <locale>` per App Store language (zh-Hant, en-US),
listing what iPhone users notice, without emoji (the App Store rejects them; asc-submit.py drops
any left). tools/asc-submit.py reads the released version's section;
a version without one gets a generic "Bug fixes and improvements" line. Delete old sections.
-->

## 2.41.0
### zh-Hant
- 新增團隊任務板：在 bot 列表點清單按鈕，就能新增任務、指派給 bot，並查看 bot 認領、記錄和完成的進度。git 專案裡的任務會各自用獨立的分支工作，完成後可以直接在任務板上合併。
- 主帥 bot 會把大任務拆成子任務分給其他 bot 同時進行，完成後由另一個 bot 審查。
- 電腦上有多個 bot 同時操作時，每一個都會顯示正在使用電腦的標記。
### en-US
- New team board: tap the checklist button on the bot list to add tasks, assign them to bots, and follow as bots claim, note and finish them. Tasks in a git project each work on their own branch, which you merge right from the board.
- Your lead bot splits a big task into subtasks for the others to work on at once, and another bot checks each finished task.
- Every bot that is using the computer at the same time now shows the in-use marker, not just one.

## 2.40.3
### zh-Hant
- bot 現在可以讓 dev server 持續在電腦上執行，不會因為 bot 閒置或重新啟動而中斷。port 會顯示在聊天標題下方；點 bot 的名稱，就能看記錄、重新啟動或停止；手機連得到電腦時，也能直接開啟。
- 用量頁面可以自己選要看哪些服務並調整順序：預設只顯示 bot 正在用的，也可以打開或隱藏其他服務，或拖曳排列。另外支援顯示 Google Antigravity 和 GitHub Copilot 的額度。
- 修正 bot 更新時，選單偶爾點了沒反應的問題。
- 修正 iOS 27.2 之後的部分測試版上，登入後 app 當掉，之後每次開啟都立刻關閉（重新安裝也一樣）的問題。

### en-US
- Bots can now keep a dev server running on your computer, even when the bot goes idle or restarts. Its port shows under the chat title; tap the bot's name to read its logs, restart or stop it, and to open it when your phone can reach it.
- Choose and arrange which agents show in Usage: by default it lists the ones your bots run, and you can turn others on, hide them or drag them into order. Usage now also reads Google Antigravity's and GitHub Copilot's limits.
- Fixed menus sometimes ignoring a tap while a bot was updating.
- Fixed a crash on some betas of iOS 27.2 and later that closed the app after sign-in and at every launch afterwards, even after reinstalling.
