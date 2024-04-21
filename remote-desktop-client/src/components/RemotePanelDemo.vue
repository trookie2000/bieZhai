<template>
  <div class="container">
    <div class="sidebar">
      <!-- 视频名字列表，点击时全屏显示对应的视频 -->
      <ul>
        <li v-for="(video, index) in videos" :key="index" @click="toggleFullScreen(index)">
          Video {{ index + 1 }} - {{ video.name }}
        </li>
      </ul>
    </div>
    <div class="main-content">
      <div class="video-grid">
        <div v-for="(video, index) in videos" :key="index" class="video-container">
          <video ref="videoElements" controls autoplay></video>
        </div>
      </div>
      <button @click="addVideo">添加视频</button>
    </div>
  </div>
</template>

<script>
export default {
  data() {
    return {
      videos: [],
      constraints: { video: true, audio: false },
      pcConfig: { iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] },
      pc: null
    };
  },
  methods: {
    async addVideo() {
      try {
        const stream = await navigator.mediaDevices.getDisplayMedia(this.constraints);
        console.log('Stream:', stream);

        if (!this.pc) {
          this.createPeerConnection();
        }

        const videoTracks = stream.getVideoTracks();
        console.log('Video tracks:', videoTracks);

        if (videoTracks.length === 0) {
          console.error('No video tracks found in stream.');
          return;
        }

        const track = videoTracks[0];
        const videoName = `Video ${this.videos.length + 1}`;
        track.onended = () => {
          this.videos = this.videos.filter(v => v.stream !== stream);
        };

        this.pc.addTrack(track, stream);
        this.videos.push({ stream, name: videoName });

        this.$nextTick(() => {
          const elems = this.$refs.videoElements;
          const elem = elems[elems.length - 1];
          if (elem) {
            elem.srcObject = stream;
          } else {
            console.error('Video element not found');
          }
        });
      } catch (error) {
        console.error('Error adding video:', error);
      }
    },

    createPeerConnection() {
      this.pc = new RTCPeerConnection(this.pcConfig);
    },

    toggleFullScreen(index) {
      const elems = this.$refs.videoElements;
      const elem = elems[index];
      if (!document.fullscreenElement && elem) {
        elem.requestFullscreen().catch(err => {
          console.error(`Error attempting to enable full-screen mode: ${err.message} (${err.name})`);
        });
      } else {
        if (document.fullscreenElement === elem) {
          document.exitFullscreen();
        }
      }
    }
  }
};
</script>

<style scoped>
.container {
  display: flex;
}

.sidebar {
  width: 200px;
  background-color: #f4f4f4;
  padding: 10px;
}

.main-content {
  flex-grow: 1;
}

.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  grid-gap: 10px;
}

.video-container {
  position: relative;
  width: 100%;
  padding-top: 56.25%; /* 16:9 Aspect Ratio */
}

video {
  position: absolute;
  width: 100%;
  height: 100%;
  top: 0;
  left: 0;
}
</style>
