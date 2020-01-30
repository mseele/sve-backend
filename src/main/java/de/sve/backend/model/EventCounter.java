package de.sve.backend.model;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class EventCounter {

	public static EventCounter create(Event event) {
		return new AutoValue_EventCounter(event.id(), event.maxSubscribers(), event.subscribers(), event.waitingList(), event.maxWaitingList());
	}

	public abstract String id();

	public abstract Long maxSubscribers();

	public abstract Long subscribers();

	public abstract Long waitingList();

	public abstract Long maxWaitingList();

}
