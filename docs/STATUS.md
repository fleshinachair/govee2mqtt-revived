# What is and is not fixed

Numbers below are issues and pull requests in
[wez/govee2mqtt](https://github.com/wez/govee2mqtt), as open on 2026-11-27.

**"Fixed here" does not mean confirmed.** Almost nothing has been tried against
the reporter's hardware or account. If one of these is yours,
[tell us what happens](https://github.com/fleshinachair/govee2mqtt-revived/issues).

All 324 open issues have been sorted below. Most were judged from the title, the
start of the report and the latest replies, and were not reproduced.

## Confirmed working

| Problem | Issues |
|---|---|
| H607C scenes missing: they are listed on the owner's lamp, and a separate fault found later, where the wrong effect played, is fixed there too | [#286](https://github.com/wez/govee2mqtt/issues/286) |
| Tap-to-Run shortcuts not working: a reporter on a Raspberry Pi saw every one work after pressing Purge Caches | [#737](https://github.com/wez/govee2mqtt/issues/737) |
| Offline lights still shown as on (opt-in `offline_unavailable`): a reporter saw seven H601C become unavailable. Becoming available again is not yet tested | [#551](https://github.com/wez/govee2mqtt/issues/551) |

## Fixed here, not confirmed

| Area | Issues |
|---|---|
| Startup crashes and outages | [#76](https://github.com/wez/govee2mqtt/issues/76), [#536](https://github.com/wez/govee2mqtt/issues/536), [#538](https://github.com/wez/govee2mqtt/issues/538), [#549](https://github.com/wez/govee2mqtt/issues/549), [#565](https://github.com/wez/govee2mqtt/issues/565), [#578](https://github.com/wez/govee2mqtt/issues/578), [#617](https://github.com/wez/govee2mqtt/issues/617), [#618](https://github.com/wez/govee2mqtt/issues/618), [#732](https://github.com/wez/govee2mqtt/issues/732) |
| Login, two-step verification, Tap-to-Run | [#627](https://github.com/wez/govee2mqtt/issues/627), [#633](https://github.com/wez/govee2mqtt/issues/633), [#647](https://github.com/wez/govee2mqtt/issues/647), [#682](https://github.com/wez/govee2mqtt/issues/682), [#702](https://github.com/wez/govee2mqtt/issues/702), [#745](https://github.com/wez/govee2mqtt/issues/745) |
| Scenes and effects | [#666](https://github.com/wez/govee2mqtt/issues/666), [#722](https://github.com/wez/govee2mqtt/issues/722) |
| MQTT and availability | [#110](https://github.com/wez/govee2mqtt/issues/110), [#227](https://github.com/wez/govee2mqtt/issues/227), [#572](https://github.com/wez/govee2mqtt/issues/572), [#659](https://github.com/wez/govee2mqtt/issues/659), [#661](https://github.com/wez/govee2mqtt/issues/661), [#686](https://github.com/wez/govee2mqtt/issues/686), [#712](https://github.com/wez/govee2mqtt/issues/712), [#719](https://github.com/wez/govee2mqtt/issues/719) |
| Offline devices (opt-in setting) | [#465](https://github.com/wez/govee2mqtt/issues/465), [#509](https://github.com/wez/govee2mqtt/issues/509), [#563](https://github.com/wez/govee2mqtt/issues/563), [#359](https://github.com/wez/govee2mqtt/issues/359), [#387](https://github.com/wez/govee2mqtt/issues/387) |
| Colour temperature | [#558](https://github.com/wez/govee2mqtt/issues/558), [#591](https://github.com/wez/govee2mqtt/issues/591), [#731](https://github.com/wez/govee2mqtt/issues/731), [#742](https://github.com/wez/govee2mqtt/issues/742) |
| Models and features | [#116](https://github.com/wez/govee2mqtt/issues/116), [#186](https://github.com/wez/govee2mqtt/issues/186), [#343](https://github.com/wez/govee2mqtt/issues/343), [#416](https://github.com/wez/govee2mqtt/issues/416), [#456](https://github.com/wez/govee2mqtt/issues/456), [#577](https://github.com/wez/govee2mqtt/issues/577), [#605](https://github.com/wez/govee2mqtt/issues/605), [#621](https://github.com/wez/govee2mqtt/issues/621), [#634](https://github.com/wez/govee2mqtt/issues/634), [#561](https://github.com/wez/govee2mqtt/issues/561), [#684](https://github.com/wez/govee2mqtt/issues/684) |
| Crash on non-English preset names | [#603](https://github.com/wez/govee2mqtt/issues/603), [#607](https://github.com/wez/govee2mqtt/issues/607), [#609](https://github.com/wez/govee2mqtt/issues/609) |
| Sign-in refused, "app version is too low" | [#626](https://github.com/wez/govee2mqtt/issues/626), [#637](https://github.com/wez/govee2mqtt/issues/637), [#649](https://github.com/wez/govee2mqtt/issues/649) |
| Home Assistant rejects a light with no colour modes | [#497](https://github.com/wez/govee2mqtt/issues/497), [#589](https://github.com/wez/govee2mqtt/issues/589), [#748](https://github.com/wez/govee2mqtt/issues/748) |
| Changing the web page port or address | [#200](https://github.com/wez/govee2mqtt/issues/200), [#424](https://github.com/wez/govee2mqtt/issues/424) |
| Sensor values shown as `{"value": 6}` | [#510](https://github.com/wez/govee2mqtt/issues/510), [#555](https://github.com/wez/govee2mqtt/issues/555) |
| No native fan entity | [#87](https://github.com/wez/govee2mqtt/issues/87), [#262](https://github.com/wez/govee2mqtt/issues/262) |
| Passwords from Docker secrets | [#362](https://github.com/wez/govee2mqtt/issues/362) |
| Repeated requests after "too many requests" | [#542](https://github.com/wez/govee2mqtt/issues/542) |
| DreamView entries causing errors or hiding Tap-to-Run | [#317](https://github.com/wez/govee2mqtt/issues/317), [#352](https://github.com/wez/govee2mqtt/issues/352), [#381](https://github.com/wez/govee2mqtt/issues/381) |
| Scene list that could not be read | [#324](https://github.com/wez/govee2mqtt/issues/324) |
| Music mode palettes | [#705](https://github.com/wez/govee2mqtt/issues/705) |
| Project maintenance | [#700](https://github.com/wez/govee2mqtt/issues/700), [#746](https://github.com/wez/govee2mqtt/issues/746) |
| Number sliders Home Assistant rejected | [#154](https://github.com/wez/govee2mqtt/issues/154), [#171](https://github.com/wez/govee2mqtt/issues/171) |
| Older lights ignoring commands sent over AWS IoT (opt-in setting) | [#113](https://github.com/wez/govee2mqtt/issues/113), [#125](https://github.com/wez/govee2mqtt/issues/125), [#152](https://github.com/wez/govee2mqtt/issues/152), [#319](https://github.com/wez/govee2mqtt/issues/319), [#340](https://github.com/wez/govee2mqtt/issues/340), [#364](https://github.com/wez/govee2mqtt/issues/364) |
| Heaters losing Auto Stop (a new switch) | [#84](https://github.com/wez/govee2mqtt/issues/84), [#131](https://github.com/wez/govee2mqtt/issues/131) |
| H61C3 staying off when given a brightness or colour (the report there is about an H618C, which is unchanged) | [#521](https://github.com/wez/govee2mqtt/issues/521) |
| Battery level for sensors (needs the Govee account) | [#646](https://github.com/wez/govee2mqtt/issues/646), [#668](https://github.com/wez/govee2mqtt/issues/668) |
| How often sensors are read (new setting) | [#672](https://github.com/wez/govee2mqtt/issues/672) |
| Models added or read in the right unit: H604A, H600A, H61E6, H713C | [#136](https://github.com/wez/govee2mqtt/issues/136), [#191](https://github.com/wez/govee2mqtt/issues/191), [#307](https://github.com/wez/govee2mqtt/issues/307), [#515](https://github.com/wez/govee2mqtt/issues/515), [#740](https://github.com/wez/govee2mqtt/issues/740) |
| TLS to the MQTT broker (new settings) | [#257](https://github.com/wez/govee2mqtt/issues/257) |
| MQTT topics, now [documented](MQTT_TOPICS.md) | [#102](https://github.com/wez/govee2mqtt/issues/102) |
| Deleted devices coming back (new setting to ignore them) | [#333](https://github.com/wez/govee2mqtt/issues/333), [#436](https://github.com/wez/govee2mqtt/issues/436), [#721](https://github.com/wez/govee2mqtt/issues/721) |
| The web page inside Home Assistant, behind its sign-in | [#306](https://github.com/wez/govee2mqtt/issues/306) |

Some of these are only *expected* to be fixed, because a change addresses the
cause the report describes.

## Needs someone with the device or account

| Problem | Issues |
|---|---|
| "Timed out waiting for status" over the LAN | [#707](https://github.com/wez/govee2mqtt/issues/707) |
| Scenes accepted but not applied | [#729](https://github.com/wez/govee2mqtt/issues/729), [#735](https://github.com/wez/govee2mqtt/issues/735) |
| H6022 stops responding altogether with LAN control on | [#518](https://github.com/wez/govee2mqtt/issues/518) |
| Air purifier readings missing | [#369](https://github.com/wez/govee2mqtt/issues/369), [#667](https://github.com/wez/govee2mqtt/issues/667) |
| State does not update after a command | [#508](https://github.com/wez/govee2mqtt/issues/508) |
| Slow cloud control | [#623](https://github.com/wez/govee2mqtt/issues/623) |
| Broker "not specified" or unavailable at boot | [#142](https://github.com/wez/govee2mqtt/issues/142), [#402](https://github.com/wez/govee2mqtt/issues/402), [#408](https://github.com/wez/govee2mqtt/issues/408), [#486](https://github.com/wez/govee2mqtt/issues/486), [#631](https://github.com/wez/govee2mqtt/issues/631) |
| IoT connection problems | [#696](https://github.com/wez/govee2mqtt/issues/696) |
| Segments missing, slow or not responding | [#105](https://github.com/wez/govee2mqtt/issues/105), [#117](https://github.com/wez/govee2mqtt/issues/117), [#207](https://github.com/wez/govee2mqtt/issues/207), [#260](https://github.com/wez/govee2mqtt/issues/260), [#399](https://github.com/wez/govee2mqtt/issues/399), [#400](https://github.com/wez/govee2mqtt/issues/400), [#477](https://github.com/wez/govee2mqtt/issues/477), [#533](https://github.com/wez/govee2mqtt/issues/533), [#706](https://github.com/wez/govee2mqtt/issues/706), [#744](https://github.com/wez/govee2mqtt/issues/744) |
| Parts of one light cannot be controlled separately | [#327](https://github.com/wez/govee2mqtt/issues/327), [#493](https://github.com/wez/govee2mqtt/issues/493) |
| DIY, shared or spliced scenes and snapshots missing | [#83](https://github.com/wez/govee2mqtt/issues/83), [#165](https://github.com/wez/govee2mqtt/issues/165), [#195](https://github.com/wez/govee2mqtt/issues/195), [#274](https://github.com/wez/govee2mqtt/issues/274), [#285](https://github.com/wez/govee2mqtt/issues/285), [#290](https://github.com/wez/govee2mqtt/issues/290), [#302](https://github.com/wez/govee2mqtt/issues/302), [#318](https://github.com/wez/govee2mqtt/issues/318), [#325](https://github.com/wez/govee2mqtt/issues/325), [#335](https://github.com/wez/govee2mqtt/issues/335), [#527](https://github.com/wez/govee2mqtt/issues/527), [#532](https://github.com/wez/govee2mqtt/issues/532), [#570](https://github.com/wez/govee2mqtt/issues/570), [#727](https://github.com/wez/govee2mqtt/issues/727) |
| Scenes or colours behave wrongly | [#128](https://github.com/wez/govee2mqtt/issues/128), [#258](https://github.com/wez/govee2mqtt/issues/258), [#298](https://github.com/wez/govee2mqtt/issues/298), [#303](https://github.com/wez/govee2mqtt/issues/303), [#304](https://github.com/wez/govee2mqtt/issues/304), [#368](https://github.com/wez/govee2mqtt/issues/368), [#447](https://github.com/wez/govee2mqtt/issues/447) |
| Light does not respond, or shows the wrong state | [#119](https://github.com/wez/govee2mqtt/issues/119), [#157](https://github.com/wez/govee2mqtt/issues/157), [#203](https://github.com/wez/govee2mqtt/issues/203), [#270](https://github.com/wez/govee2mqtt/issues/270), [#281](https://github.com/wez/govee2mqtt/issues/281), [#309](https://github.com/wez/govee2mqtt/issues/309), [#337](https://github.com/wez/govee2mqtt/issues/337), [#361](https://github.com/wez/govee2mqtt/issues/361), [#403](https://github.com/wez/govee2mqtt/issues/403), [#426](https://github.com/wez/govee2mqtt/issues/426), [#431](https://github.com/wez/govee2mqtt/issues/431), [#462](https://github.com/wez/govee2mqtt/issues/462), [#467](https://github.com/wez/govee2mqtt/issues/467), [#485](https://github.com/wez/govee2mqtt/issues/485), [#514](https://github.com/wez/govee2mqtt/issues/514), [#568](https://github.com/wez/govee2mqtt/issues/568), [#576](https://github.com/wez/govee2mqtt/issues/576), [#586](https://github.com/wez/govee2mqtt/issues/586) |
| Heaters, humidifiers, kettles, purifiers and fans | [#107](https://github.com/wez/govee2mqtt/issues/107), [#109](https://github.com/wez/govee2mqtt/issues/109), [#129](https://github.com/wez/govee2mqtt/issues/129), [#134](https://github.com/wez/govee2mqtt/issues/134), [#140](https://github.com/wez/govee2mqtt/issues/140), [#145](https://github.com/wez/govee2mqtt/issues/145), [#149](https://github.com/wez/govee2mqtt/issues/149), [#169](https://github.com/wez/govee2mqtt/issues/169), [#201](https://github.com/wez/govee2mqtt/issues/201), [#300](https://github.com/wez/govee2mqtt/issues/300), [#413](https://github.com/wez/govee2mqtt/issues/413), [#438](https://github.com/wez/govee2mqtt/issues/438), [#444](https://github.com/wez/govee2mqtt/issues/444), [#520](https://github.com/wez/govee2mqtt/issues/520), [#657](https://github.com/wez/govee2mqtt/issues/657), [#709](https://github.com/wez/govee2mqtt/issues/709), [#713](https://github.com/wez/govee2mqtt/issues/713) |
| Models not supported yet | [#114](https://github.com/wez/govee2mqtt/issues/114), [#173](https://github.com/wez/govee2mqtt/issues/173), [#184](https://github.com/wez/govee2mqtt/issues/184), [#210](https://github.com/wez/govee2mqtt/issues/210), [#221](https://github.com/wez/govee2mqtt/issues/221), [#259](https://github.com/wez/govee2mqtt/issues/259), [#284](https://github.com/wez/govee2mqtt/issues/284), [#329](https://github.com/wez/govee2mqtt/issues/329), [#338](https://github.com/wez/govee2mqtt/issues/338), [#357](https://github.com/wez/govee2mqtt/issues/357), [#374](https://github.com/wez/govee2mqtt/issues/374), [#384](https://github.com/wez/govee2mqtt/issues/384), [#409](https://github.com/wez/govee2mqtt/issues/409), [#580](https://github.com/wez/govee2mqtt/issues/580), [#677](https://github.com/wez/govee2mqtt/issues/677) |

Also unconfirmed: what happens when the login token expires on an account with
two-step verification.

## Might be helped

A change here touches the problem, but may not solve all of it: [#67](https://github.com/wez/govee2mqtt/issues/67), [#100](https://github.com/wez/govee2mqtt/issues/100), [#139](https://github.com/wez/govee2mqtt/issues/139), [#144](https://github.com/wez/govee2mqtt/issues/144), [#159](https://github.com/wez/govee2mqtt/issues/159), [#174](https://github.com/wez/govee2mqtt/issues/174), [#185](https://github.com/wez/govee2mqtt/issues/185), [#189](https://github.com/wez/govee2mqtt/issues/189), [#193](https://github.com/wez/govee2mqtt/issues/193), [#197](https://github.com/wez/govee2mqtt/issues/197), [#199](https://github.com/wez/govee2mqtt/issues/199), [#231](https://github.com/wez/govee2mqtt/issues/231), [#238](https://github.com/wez/govee2mqtt/issues/238), [#242](https://github.com/wez/govee2mqtt/issues/242), [#269](https://github.com/wez/govee2mqtt/issues/269), [#272](https://github.com/wez/govee2mqtt/issues/272), [#276](https://github.com/wez/govee2mqtt/issues/276), [#282](https://github.com/wez/govee2mqtt/issues/282), [#283](https://github.com/wez/govee2mqtt/issues/283), [#288](https://github.com/wez/govee2mqtt/issues/288), [#293](https://github.com/wez/govee2mqtt/issues/293), [#294](https://github.com/wez/govee2mqtt/issues/294), [#297](https://github.com/wez/govee2mqtt/issues/297), [#299](https://github.com/wez/govee2mqtt/issues/299), [#311](https://github.com/wez/govee2mqtt/issues/311), [#312](https://github.com/wez/govee2mqtt/issues/312), [#320](https://github.com/wez/govee2mqtt/issues/320), [#322](https://github.com/wez/govee2mqtt/issues/322), [#323](https://github.com/wez/govee2mqtt/issues/323), [#353](https://github.com/wez/govee2mqtt/issues/353), [#358](https://github.com/wez/govee2mqtt/issues/358), [#372](https://github.com/wez/govee2mqtt/issues/372), [#376](https://github.com/wez/govee2mqtt/issues/376), [#392](https://github.com/wez/govee2mqtt/issues/392), [#393](https://github.com/wez/govee2mqtt/issues/393), [#397](https://github.com/wez/govee2mqtt/issues/397), [#441](https://github.com/wez/govee2mqtt/issues/441), [#442](https://github.com/wez/govee2mqtt/issues/442), [#457](https://github.com/wez/govee2mqtt/issues/457), [#466](https://github.com/wez/govee2mqtt/issues/466), [#506](https://github.com/wez/govee2mqtt/issues/506), [#571](https://github.com/wez/govee2mqtt/issues/571), [#636](https://github.com/wez/govee2mqtt/issues/636), [#708](https://github.com/wez/govee2mqtt/issues/708).

## Reported more than once, still open

Each row looks like one underlying problem. This is judged from the reports and
has not been proven. DreamView actions in Tap-to-Run cannot be run, because
Govee gives no command for them.

| Problem | Issues |
|---|---|
| Wrong segment count, or segments that do not respond | [#473](https://github.com/wez/govee2mqtt/issues/473), [#543](https://github.com/wez/govee2mqtt/issues/543), [#545](https://github.com/wez/govee2mqtt/issues/545), [#701](https://github.com/wez/govee2mqtt/issues/701), [#710](https://github.com/wez/govee2mqtt/issues/710), [#728](https://github.com/wez/govee2mqtt/issues/728), [#733](https://github.com/wez/govee2mqtt/issues/733) |
| Multi-outlet plugs appear as one switch | [#65](https://github.com/wez/govee2mqtt/issues/65), [#332](https://github.com/wez/govee2mqtt/issues/332), [#443](https://github.com/wez/govee2mqtt/issues/443) |
| Tap-to-Run listed but does nothing useful | [#406](https://github.com/wez/govee2mqtt/issues/406), [#523](https://github.com/wez/govee2mqtt/issues/523), [#635](https://github.com/wez/govee2mqtt/issues/635) |
| Govee refuses some scenes | [#582](https://github.com/wez/govee2mqtt/issues/582), [#596](https://github.com/wez/govee2mqtt/issues/596) |
| H7105 tower fan has no controls | [#237](https://github.com/wez/govee2mqtt/issues/237), [#620](https://github.com/wez/govee2mqtt/issues/620) |
| Water leak sensors have no readings | [#328](https://github.com/wez/govee2mqtt/issues/328), [#378](https://github.com/wez/govee2mqtt/issues/378) |

## Features not built

| Feature | Issues |
|---|---|
| Choosing the HDMI input on a sync box | [#479](https://github.com/wez/govee2mqtt/issues/479), [#494](https://github.com/wez/govee2mqtt/issues/494), [#513](https://github.com/wez/govee2mqtt/issues/513), [#588](https://github.com/wez/govee2mqtt/issues/588) |
| Video, game and DreamView modes | [#334](https://github.com/wez/govee2mqtt/issues/334), [#529](https://github.com/wez/govee2mqtt/issues/529), [#611](https://github.com/wez/govee2mqtt/issues/611) |
| Effect speed and autoplay | [#190](https://github.com/wez/govee2mqtt/issues/190), [#331](https://github.com/wez/govee2mqtt/issues/331) |
| A climate entity for heaters | [#166](https://github.com/wez/govee2mqtt/issues/166) |
| Showing the new state before Govee confirms it | [#695](https://github.com/wez/govee2mqtt/issues/695) |
| More than one Govee account | [#522](https://github.com/wez/govee2mqtt/issues/522) |

## Looked into, not fixed

* [#749](https://github.com/wez/govee2mqtt/issues/749): high processor use after a broker restart. It happens inside the
  bundled MQTT library (libmosquitto 2.0.9). Reading its code did not explain the
  reported trace, and it has not been reproduced. Restarting the add-on clears it.
* [#567](https://github.com/wez/govee2mqtt/issues/567): the segment count for H61E5 is unchanged.
* H6020 without a Govee API key: effects chosen over the LAN do nothing. A
  contributor tried five ways of sending them on their lamp and none was right
  ([notes](FIELD_NOTES.md#scenes-over-the-lan)).

## Pull requests held back

* [#724](https://github.com/wez/govee2mqtt/pull/724), scene speed: its author says it is AI generated, and it
  indexes into device data without bounds checks.
* [#698](https://github.com/wez/govee2mqtt/pull/698), H1310 fan: its Home Assistant checks are not ticked, and
  it overlaps the fan and light handling here.
* [#736](https://github.com/wez/govee2mqtt/pull/736), saved device identity: replaced by a smaller outage
  fallback.

## Will not be fixed here

| Reason | Issues |
|---|---|
| Govee's service | [#35](https://github.com/wez/govee2mqtt/issues/35), [#40](https://github.com/wez/govee2mqtt/issues/40), [#49](https://github.com/wez/govee2mqtt/issues/49), [#79](https://github.com/wez/govee2mqtt/issues/79), [#96](https://github.com/wez/govee2mqtt/issues/96), [#112](https://github.com/wez/govee2mqtt/issues/112), [#178](https://github.com/wez/govee2mqtt/issues/178), [#239](https://github.com/wez/govee2mqtt/issues/239), [#245](https://github.com/wez/govee2mqtt/issues/245), [#249](https://github.com/wez/govee2mqtt/issues/249), [#316](https://github.com/wez/govee2mqtt/issues/316), [#346](https://github.com/wez/govee2mqtt/issues/346) |
| Govee's API or the device itself | [#53](https://github.com/wez/govee2mqtt/issues/53), [#62](https://github.com/wez/govee2mqtt/issues/62), [#153](https://github.com/wez/govee2mqtt/issues/153), [#202](https://github.com/wez/govee2mqtt/issues/202) |
| Device firmware | [#747](https://github.com/wez/govee2mqtt/issues/747), [#750](https://github.com/wez/govee2mqtt/issues/750) |
| The user's network or broker | [#175](https://github.com/wez/govee2mqtt/issues/175), [#251](https://github.com/wez/govee2mqtt/issues/251), [#360](https://github.com/wez/govee2mqtt/issues/360), [#593](https://github.com/wez/govee2mqtt/issues/593) |
| Home Assistant dropped the platform | [#517](https://github.com/wez/govee2mqtt/issues/517) |
| Needs Bluetooth, which the bridge does not have | [#61](https://github.com/wez/govee2mqtt/issues/61), [#99](https://github.com/wez/govee2mqtt/issues/99), [#118](https://github.com/wez/govee2mqtt/issues/118), [#220](https://github.com/wez/govee2mqtt/issues/220), [#296](https://github.com/wez/govee2mqtt/issues/296), [#560](https://github.com/wez/govee2mqtt/issues/560), [#569](https://github.com/wez/govee2mqtt/issues/569), [#630](https://github.com/wez/govee2mqtt/issues/630) |
| Not offered to the bridge by Govee, as far as the report shows | [#192](https://github.com/wez/govee2mqtt/issues/192), [#685](https://github.com/wez/govee2mqtt/issues/685), [#692](https://github.com/wez/govee2mqtt/issues/692) |
| A question, a setup problem, or too little to act on | [#130](https://github.com/wez/govee2mqtt/issues/130), [#155](https://github.com/wez/govee2mqtt/issues/155), [#205](https://github.com/wez/govee2mqtt/issues/205), [#264](https://github.com/wez/govee2mqtt/issues/264), [#301](https://github.com/wez/govee2mqtt/issues/301), [#330](https://github.com/wez/govee2mqtt/issues/330), [#339](https://github.com/wez/govee2mqtt/issues/339), [#344](https://github.com/wez/govee2mqtt/issues/344), [#366](https://github.com/wez/govee2mqtt/issues/366), [#383](https://github.com/wez/govee2mqtt/issues/383), [#386](https://github.com/wez/govee2mqtt/issues/386), [#390](https://github.com/wez/govee2mqtt/issues/390), [#472](https://github.com/wez/govee2mqtt/issues/472), [#516](https://github.com/wez/govee2mqtt/issues/516), [#519](https://github.com/wez/govee2mqtt/issues/519), [#528](https://github.com/wez/govee2mqtt/issues/528), [#557](https://github.com/wez/govee2mqtt/issues/557), [#584](https://github.com/wez/govee2mqtt/issues/584), [#590](https://github.com/wez/govee2mqtt/issues/590) |

Some of these may already be closed.
