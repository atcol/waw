jq '[.auctions[]] | group_by(.item.id)' ${1}