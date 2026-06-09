<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { listen } from '@tauri-apps/api/event'

const mouthOpen = ref(0)

let unlisten: (() => void) | null = null

onMounted(async () => {
  try {
    unlisten = await listen<{ level: number }>('audio_level', e => {
      mouthOpen.value = Math.min(e.payload.level * 2.5, 1)
    })
  } catch {}
})

onBeforeUnmount(() => unlisten?.())
</script>

<template>
  <div class="avatar">
    <div class="body">
      <!-- Hair -->
      <div class="hair" />
      <!-- Head -->
      <div class="head">
        <div class="eye left"><div class="pupil" /></div>
        <div class="eye right"><div class="pupil" /></div>
        <div class="nose" />
        <div class="mouth" :style="{ transform: `scaleY(${0.3 + mouthOpen * 0.7})` }" />
        <!-- Blush -->
        <div class="blush left" />
        <div class="blush right" />
      </div>
      <!-- Torso -->
      <div class="torso">
        <div class="collar" />
      </div>
    </div>
    <div class="shadow" />
  </div>
</template>

<style scoped>
.avatar {
  width: 100%; height: 100%; min-height: 300px;
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  position: relative; overflow: hidden;
  background: linear-gradient(180deg, #e8f0fe 0%, #f0f4ff 60%, #fff 100%);
}
.body {
  position: relative;
  animation: float 3s ease-in-out infinite;
}
@keyframes float {
  0%, 100% { transform: translateY(0) }
  50% { transform: translateY(-6px) }
}
.shadow {
  width: 80px; height: 10px;
  background: rgba(0,0,0,.06);
  border-radius: 50%;
  margin-top: -4px;
  animation: shadowPulse 3s ease-in-out infinite;
}
@keyframes shadowPulse {
  0%, 100% { transform: scaleX(1); opacity: .6 }
  50% { transform: scaleX(.85); opacity: .3 }
}

/* Hair */
.hair {
  width: 140px; height: 80px;
  background: linear-gradient(180deg, #5b3a8c 0%, #7c5cbf 100%);
  border-radius: 80px 80px 0 0;
  position: absolute; top: -20px; left: 50%;
  transform: translateX(-50%);
  z-index: 3;
}
.hair::before {
  content: '';
  position: absolute; bottom: -15px; left: -18px;
  width: 40px; height: 35px;
  background: #7c5cbf; border-radius: 50%;
}
.hair::after {
  content: '';
  position: absolute; bottom: -15px; right: -18px;
  width: 40px; height: 35px;
  background: #7c5cbf; border-radius: 50%;
}

/* Head */
.head {
  width: 120px; height: 130px;
  background: linear-gradient(180deg, #ffead5 0%, #ffe0c0 100%);
  border-radius: 60px 60px 50px 50px;
  position: relative; z-index: 2;
  box-shadow: inset 0 -8px 12px rgba(0,0,0,.04);
}

/* Eyes */
.eye {
  width: 18px; height: 22px;
  background: white;
  border-radius: 50%;
  position: absolute; top: 42px;
  box-shadow: 0 1px 2px rgba(0,0,0,.08);
}
.eye.left { left: 24px }
.eye.right { right: 24px }
.pupil {
  width: 9px; height: 10px;
  background: #3a2060; border-radius: 50%;
  position: absolute; top: 7px; left: 50%;
  transform: translateX(-50%);
}
.pupil::after {
  content: ''; position: absolute;
  width: 3px; height: 3px;
  background: white; border-radius: 50%;
  top: 2px; left: 2px;
}
/* Blink animation */
.eye { animation: blink 4s infinite; }
@keyframes blink {
  0%, 92%, 100% { transform: scaleY(1) }
  94% { transform: scaleY(.05) }
}

/* Blush */
.blush {
  width: 18px; height: 10px;
  background: rgba(255,150,150,.25);
  border-radius: 50%;
  position: absolute; top: 65px;
}
.blush.left { left: 16px }
.blush.right { right: 16px }

/* Nose */
.nose {
  width: 6px; height: 4px;
  background: rgba(0,0,0,.1);
  border-radius: 50%;
  position: absolute; top: 70px; left: 50%;
  transform: translateX(-50%);
}

/* Mouth */
.mouth {
  width: 24px; height: 10px;
  background: #e88888;
  border-radius: 0 0 12px 12px;
  position: absolute; top: 82px; left: 50%;
  transform: translateX(-50%) scaleY(.3);
  transition: transform .08s;
}

/* Torso */
.torso {
  width: 100px; height: 50px;
  background: linear-gradient(180deg, #7c5cbf 0%, #5b3a8c 100%);
  border-radius: 30px 30px 0 0;
  margin: -8px auto 0;
  position: relative; z-index: 1;
}
.collar {
  width: 80px; height: 12px;
  background: #fff;
  border-radius: 6px;
  margin: 2px auto 0;
  opacity: .5;
}
</style>
