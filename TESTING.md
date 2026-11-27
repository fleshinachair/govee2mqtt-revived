# Testing the pre-release

Thank you for trying it. This fork has not confirmed any change on real Govee
hardware, so your report is the first real test. "It works" is useful too.

## Before you start

1. Take a Home Assistant backup.
2. If you are moving from wez's add-on, note how many Govee entities you have,
   then follow
   [the migration steps](README.md#migrating-from-wezgovee2mqtt-zero-entity-loss).
3. Turn on **Start on boot** and **Watchdog** for the new add-on. With Docker,
   keep `restart: unless-stopped`.

## Everyone

- [ ] Your old entities are still there, with the same names and history, and
      none are duplicated.
- [ ] Power, brightness, color and effects work.
- [ ] Restart Home Assistant. Entities become available again within a couple of
      minutes without restarting the add-on.
- [ ] Restart the MQTT broker. Control works again afterwards.
- [ ] Leave it running for a day or more. Control still works.
- [ ] The log has no `panicked`, no `FATAL`, and no repeated logins.

## If this applies to you

| You have | Try | Tell us |
|---|---|---|
| A Govee account with 2FA | [Enter the emailed code](docs/CONFIG.md#two-factor-authentication-2fa), then restart a few times | Whether it asks for a new code again |
| Tap-to-Run shortcuts | Activate one from Home Assistant | Whether they are listed and work |
| H6022, H6065, H6066, H6052, H6092, H610A, H6079 or H70C4 | Choose several effects | Whether the light actually changes |
| H66A1, or a startup failure on wez's version | Start the add-on | Whether it starts |
| A light with color temperature | Set the warmest and coolest values | Whether it stays lit, and whether the range looks right |
| A segmented light | Change individual segments | Whether the app and Home Assistant agree |
| A fan or air purifier | Use the new `fan` entity | Whether speed and modes match the device |
| H5140 or H5106 | Look at the sensors | Whether the values match the Govee app |
| H605C or H61A9 | Switch it on and off | How long it takes |
| An effects list that was empty on wez's version | Open the list | Whether effects appear |
| A device that is unplugged | Check its Status sensor after 20 minutes | What it says |
| Something already using port 8056 | Set `http_port` | Whether the web page loads |

## Newer changes

These are new since the first pre-release. The first one,
`2026.11.26-dd7edbbb`, does not have them.

| You have | Try | Tell us |
|---|---|---|
| Any light on the LAN | `light.turn_on` with `brightness` and a 30 s `transition`, then turn it off halfway | Whether it fades, and stops at once |
| H6061 or H6066 | Change and switch off single panels; switch off the whole room | Whether the right panel changes, and nothing wakes up |
| H1270 | Switch main and halo separately | Whether each works |
| H6006, H600D, H601B, H6039 or H616D | Control it | Whether it responds quickly |
| H5086 plug | Watch the energy sensors over midnight and a replug | Whether totals look right |
| H7170, H7171 or H7173 | Boil, lift off the base, keep warm | Whether the new sensors follow |
| H6098 | Video Mode on and off | What the light does |
| Ice maker or presence sensor | Trigger an event | Whether Home Assistant sees it |
| A switch that showed unknown | Switch it | Whether its state is now right |
| A Govee login configured | Restart the add-on with the internet unplugged, then plug it back in | Whether it starts, and whether live updates return within about 10 minutes |

New in the fourth pre-release:

| You have | Try | Tell us |
|---|---|---|
| Any light with colour temperature | Set the warmest and coolest values | Whether the limits and the light look right |
| Any device | Open the web page, choose **Report a problem** | Whether the report loads, shows nothing private, and the GitHub page opens filled in |
| Tap-to-Run shortcuts | Run one | Whether it still works |

New in the sixth pre-release:

| You have | Try | Tell us |
|---|---|---|
| A light that shows commands as sent but does not react | Add its model to `no_iot_control` (needs the API key) | Whether it now responds |
| A heater with Auto Stop | Turn on the **Auto Stop** switch, set the target temperature | Whether the Govee app shows Auto Stop on |
| H61C3 | While it is off, turn it on with a brightness | Whether it lights up |
| H604A | Switch it on and off many times | Whether any command is lost |
| H600A, H61E6 or H713C | Use it | Whether the controls and readings work |
| A battery sensor and a Govee account | Compare the **Battery** sensor with the app | Whether they match, and whether it changes over days |
| A broker with TLS | Set `mqtt_tls` or `mqtt_ca`, with host and port | Whether it connects |

## Reporting

From the fourth pre-release, the web page's **Report a problem**
does most of this for you. Otherwise open an
[issue](https://github.com/fleshinachair/govee2mqtt-revived/issues) with:

* The device model, such as `H6159`
* The version from the add-on's **Info** tab
* Add-on or Docker
* What you did, what you expected, what happened
* The log, from startup if you can

**Check the log for personal details before posting it.** For more detail set
`debug_level` to `govee=trace`, but trace logs can contain passwords and tokens.

## Going back

Stop the new add-on and start the old one. With Docker, restore the old image
name and run `docker compose up -d`.
