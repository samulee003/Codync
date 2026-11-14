<!--
App Store "What's New" for the iPhone app. Write it in the change that bumps the version:
a `## <version>` section with one `### <locale>` per App Store language (zh-Hant, en-US),
listing what iPhone users notice, without emoji (the App Store rejects them; asc-submit.py drops
any left). tools/asc-submit.py reads the released version's section;
a version without one gets a generic "Bug fixes and improvements" line. Delete old sections.
-->

## 2.46.1
### zh-Hant
- bot 詳細資料的「電腦」分頁改成點一下才開始顯示畫面，不會一打開就連線。
- 遠端畫面在 5 分鐘沒有碰 App 後會自動暫停以節省流量，點一下就能繼續，原本的「接手控制」或「交還給 Bot」都會保留。
- 權限核准卡片與連線請求卡片現在會跟著 app 的語言顯示。
- 新增團隊任務板：在 bot 列表點清單按鈕，就能新增任務、指派給 bot，並查看 bot 認領、記錄和完成的進度。git 專案裡的任務會各自用獨立的分支工作，完成後可以直接在任務板上合併。
- 任務板會等你決定：bot 提出計畫時，可以在任務上核准，或說明要改什麼；bot 有問題時，會在「等你處理」區塊回答。新增任務時可以寫驗收條件，完成後每一條會顯示是否通過。
- 主帥 bot 會把大任務拆成子任務分給其他 bot 同時進行，完成後由另一個 bot 審查。
- 電腦上有多個 bot 同時操作時，每一個都會顯示正在使用電腦的標記。

### en-US
- The Computer tab in bot details starts its picture when you tap it, instead of connecting as soon as it opens.
- A computer's screen pauses after 5 minutes without you touching the app, to save data. Tap to continue; your Take over or Hand back to bots choice stays.
- Approval and connection request cards now follow the app's language.
- New team board: tap the checklist button on the bot list to add tasks, assign them to bots, and follow as bots claim, note and finish them. Tasks in a git project each work on their own branch, which you merge right from the board.
- The team board waits for you: when a bot proposes a plan, approve it or ask for changes; when it has questions, answer them under Waiting for you. New tasks take acceptance criteria, and each one shows whether it passed.
- Your lead bot splits a big task into subtasks for the others to work on at once, and another bot checks each finished task.
- Every bot that is using the computer at the same time now shows the in-use marker, not just one.
