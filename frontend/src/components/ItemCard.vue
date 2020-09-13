<template>

  <v-card
    class="mt-4 mx-auto"
    max-width="400"
  >
    <v-card-title>
      <div class="title font-weight-medium mb-2">{{ name }} ({{ id }})</div>
    </v-card-title>
    <v-divider class="my-4"></v-divider>
    <v-card-text class="pt-0">
      <v-list dense>
          <v-list-item>
            <v-list-item-content >Price:</v-list-item-content>
            <v-list-item-content class="align-end" ><v-chip dense color="blue" outlined>{{ `${toGold(data[0].value)}g ${toSilver(data[0].value)}s` }}</v-chip></v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >Timestamp:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ new Date(data[0].ts * 1000).toISOString() }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >High:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ max[1] }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content ></v-list-item-content>
            <v-list-item-content class="align-end" >{{ new Date(max[0] * 1000).toISOString() }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content >Low:</v-list-item-content>
            <v-list-item-content class="align-end" >{{ min[1] }}</v-list-item-content>
          </v-list-item>
          <v-list-item>
            <v-list-item-content ></v-list-item-content>
            <v-list-item-content class="align-end" >{{ new Date(min[0] * 1000).toISOString() }}</v-list-item-content>
          </v-list-item>
        </v-list>
      <v-sheet
        class="v-sheet--offset mx-auto"
        color="white"
        elevation="2"
        max-width="calc(100% - 32px)"
      >
        <v-sparkline
          :key="data.value"
          :smooth="18"
          :gradient="['#f72047', '#ffd200', '#1feaea']"
          :line-width="3"
          :value="data"
          auto-draw
          stroke-linecap="round"
        ></v-sparkline>
      </v-sheet>
    </v-card-text>
    <div :id="`chart_${id}`" />
  </v-card>

</template>

<script lang="ts">
import Vue from "vue";
import * as d3 from "d3";

interface Price {
  ts: number;
  value: number;
}

export default Vue.extend({
  name: "item-card",
  data: function () {
    return {
      chartId: this.id,
      latestPrice: this.data[this.data.length - 1].value,
      hover: false,
    }
  },

  methods: {
    toGold (val) {
      return Math.floor(val / 1000);
    },
    toSilver (val) {
      return val % 100;
    },
    toCopper (val) {
      return val / 100;
    }
  },

  props: {
    data: Array,
    name: String,
    id: Number,
    min: Array,
    max: Array,
  }
});
</script>
