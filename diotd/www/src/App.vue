<template>
  <h1>Device list</h1>
  <transition-group name="node-list" tag="div" class="nodes-container">
    <Node
      v-for="(nodeData, nodeId) in peers" :key="nodeId"
      :name="nodeData.name"
      :id="nodeId"
      :devices="nodeData.devices"></Node>
  </transition-group>
</template>

<script>
import Node from './components/Node.vue'

export default {
  name: 'App',
  data: function() {
    return {
      wsConnection: null,
      peers: {}
    }
  },
  mounted: function() {
    this.wsConnection = new WebSocket('ws://' + location.host + "/updates");

    this.wsConnection.onmessage = (msgJson) => {
      console.log(msgJson.data);
      const msg = JSON.parse(msgJson.data);
      if (msg.peers) {
        this.peers = msg.peers;
        return;
      }

      const data = msg.data;

      switch (msg.event) {
        case "sensor_data": {
          const node = data.node;
          const device = data.device;
          const sensorName = data.sensor_name;
          const value = data.value;

          if (!(node in this.peers)) {
            return;
          }

          let ownNode = this.peers[node];

          if (!(device in ownNode.devices)) {
            return;
          }

          let ownDevice = ownNode.devices[device];

          if (!(sensorName in ownDevice.sensors)) {
            ownDevice.sensors[sensorName] = {current_value: value};
          } else {
            if (value === "signal") {
              ownDevice.sensors[sensorName].current_value = { signal: Date.now() };
            } else {
              ownDevice.sensors[sensorName].current_value = value;
            }
            
          }

          break;
        }
        case "peer_identity": {
          let node = data.node;

          if (!(node in this.peers)) {
            for (let device in data.devices) {
              data.devices[device].sensors = {}
            }
            this.peers[node] = data;
          } else {
            let existingNode = this.peers[node];
            
            if (existingNode.name != data.name)
              existingNode.name = data.name;
            
            for (let device in data.devices) {
              if (device in existingNode.devices) {
                let oldDev = existingNode.devices[device];
                let newDev = data.devices[device];
                if (oldDev.type != newDev.type)
                  oldDev.type = newDev.type;
              } else {
                data.devices[device].sensors = {}
                existingNode.devices[device] = data.devices[device];
              }
            }
          }
          break;
        }
      }

      console.log(this.peers);
    };
  },
  components: {
    Node
  }
}
</script>

<style>
body {
  padding: 0;
  margin: 0;
}

#app {
  font-family: Avenir, Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  text-align: center;
  color: #2c3e50;
}

.node-list-item {
  display: inline-block;
  margin-right: 10px;
}
.node-list-enter-active,
.node-list-leave-active {
  transition: all 1s ease;
}
.node-list-enter-from,
.node-list-leave-to {
  opacity: 0;
  transform: translateY(30px);
}
</style>
