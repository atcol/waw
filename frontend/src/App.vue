<template>
  <v-app id="inspire">
    <v-app-bar app flat dark>
      <v-spacer />
      <v-autocomplete
        v-model="select"
        :items="searchResults"
        :search-input.sync="search"
        class="black--text mx-4"
        label="Search items"
        append-icon="mdi-search"
        cache-items
        hide-no-data
        hide-details
      ></v-autocomplete>
    </v-app-bar>
    <v-navigation-drawer v-model="drawer" dark permanent app>
      <v-list dense>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title class="title">Waw</v-list-item-title>
            <v-list-item-subtitle>WOW Auction Watcher</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-group prepend-icon="mdi-incognito" value="true">
          <template v-slot:activator>
            <v-list-item-title>Watchlist</v-list-item-title>
          </template>

          <v-list-item
            v-for="(symbol, i) in chartData"
            :key="String(i)"
            link
            @click="$emit('show-chart-dialog', symbol);"
          >
            <v-list-item-title v-text="symbol.name"></v-list-item-title>
          </v-list-item>
        </v-list-group>
        <v-list-item link>
          <v-list-item-action>
            <v-icon>mdi-email</v-icon>
          </v-list-item-action>
          <v-list-item-content>
            <v-list-item-title>Contact</v-list-item-title>
          </v-list-item-content>
        </v-list-item>
      </v-list>
    </v-navigation-drawer>

    <v-main>
      <v-container class="fill-height" fluid>
        <v-row align="center" justify="center">
          <v-col class="text-center">
            <div v-for="data in chartData" v-bind:key="data">
              <item-card :name="data.name" :data="data.prices" :id="data.id"></item-card>
            </div>
          </v-col>
        </v-row>
      </v-container>
    </v-main>

    <v-row justify="center">
      <v-dialog v-on:show-chart-dialog="chartDialog.open" persistent max-width="1280">
        <v-card>
          <v-card-title class="headline" v-text="chartDialog.name"></v-card-title>
          <v-card-text>
            <chart :name="chartDialog.name" :data="chartDialog.prices" :id="chartDialog.id" />
          </v-card-text>
          <v-card-actions>
            <v-spacer></v-spacer>
            <v-btn color="green darken-1" text @click="chartDialog.open = false">Close</v-btn>
          </v-card-actions>
        </v-card>
      </v-dialog>
    </v-row>
    <v-footer light>
      <span class="black--text">&copy; {{ new Date().getFullYear() }}</span>
    </v-footer>
  </v-app>
</template>

<script lang="ts">
import Vue from "vue";
import Chart from "./components/Chart.vue";
import ItemCard from "./components/ItemCard.vue";
import * as d3 from "d3";

const symbols = Vue.observable([]);

export default Vue.extend({
  name: "App",

  components: {
    chart: Chart,
    "item-card": ItemCard
  },

  watch: {},

  data: () => {
    return {
      chartData: symbols,
      searchResults: [],
      chartDialog: {}
    };
  },

  mounted() {
    this.$on("show-chart-dialog", function(i) {
      this.chartDialog = i;
      this.chartDialog.open = true;
    });
    fetch("http://winston:8080/watchlist")
      .then(data => data.json())
      .then((data: number[]) => {
        data.forEach(id => {
          d3.json("http://winston:8080/series/" + id).then(function(data: {
            prices: { value: number }[];
          }) {
            symbols.push({ values: data.prices.map(p => p.value), ...data });
          });
        });
      });
  }
});
</script>
