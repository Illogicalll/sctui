# sctui

a soundcloud client for the terminal

![demo](./media/playing_demo.png)

## Features

### 🎧 High Quality Ad-Free playback

<p align="center">Stream tracks directly from SoundCloud without the interruptions you would usually experience with a free account on the website</p>

### ✅ Fully Featured

<p align="center">Browse your own Likes, Playlists and Saved Albums as well as the Tracks and Likes of the People you follow</p>

<p align="center"><img src="./media/browse.png" alt="Browse" width="480" /></p>

<p align="center">Search for Tracks, Albums and Playlists to add to your library, as well as new People to follow</p>

<p align="center"><img src="./media/search_feature.png" alt="Search" width="480" /></p>

<p align="center">View the activity of everyone you follow to stay up to date with their latest releases or reposts</p>

<p align="center">🚧 Feature Coming Soon 🚧</p>

### 🔊 Gapless Playback

<p align="center">Enjoy seamless transitions in your favourite albums without the buffering present on SoundCloud Web</p>

### 👁️ Audio Visualiser

<p align="center">View the waveforms of your favourite music in oscilloscope or audio spectrum visualisation modes</p>

<p align="center"><img src="./media/visualiser.gif" alt="Visualiser" width="480" /></p>

<p align="center"><img src="./media/spectrum.gif" alt="Visualiser 2" width="480" /></p>

<p align="center">(maybe more modes coming soon..)</p>

## Limitations

### ❌ Playback of Go+ Tracks

- Due to SoundCloud API limitations, Go+ tracks are not playable from the application

### ❌ Downloads

- Due to the SoundCloud API Terms of Use, the download and offline playback of tracks is not supported

## Privacy

sctui talks to SoundCloud directly for everything except login. Login and hourly token refresh go through a small relay (source in [`worker/`](./worker)) that adds the app secret and passes SoundCloud's reply straight back without reading or storing it. Your password never leaves soundcloud.com. You can revoke sctui's access at any time from your SoundCloud settings.

## Dev Diary

find the dev diary to follow along the development ~~struggle~~ process [here](./DEV_DIARY.md)

## License

Copyright (c) Will Murphy <contact@w-murphy.com>

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE
