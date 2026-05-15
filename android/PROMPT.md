# Compact Handoff Prompt

Copy-paste this to continue Room database implementation:

---

**Task:** Implement Room database for arrival/departure persistence in Android bus arrival app.

**Context:**
- Branch: `feature/android-app`
- 11/13 tasks complete. Pipeline + service done.
- Next: Room entities (ArrivalEntity, DepartureEntity), DAOs, repository.

**Create:**
1. `app/src/main/java/com/busarrival/app/data/local/db/AppDatabase.kt`
2. `entity/ArrivalEntity.kt`, `entity/DepartureEntity.kt`
3. `dao/ArrivalDao.kt`, `dao/DepartureDao.kt`
4. `repository/DetectionRepository.kt`

**Requirements:**
- Entities: timestamp, stopIndex, sCm, probability/dwellTimeS, routeId
- DAOs: insert, query by time range, query by route, recent N
- Repository: Flow-based queries, bridge to DetectionService
- Tests: unit + instrumented

**Reference types:** `app/.../domain/model/StateModels.kt` (ArrivalEvent, DepartureEvent)

**See:** `android/HANDOFF.md` for full context.

**Commands:** `cd android && ./gradlew build && ./gradlew test`

---

End handoff.
