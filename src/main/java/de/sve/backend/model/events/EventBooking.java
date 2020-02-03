package de.sve.backend.model.events;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class EventBooking {

	public abstract String eventId();

	public abstract String firstName();

	public abstract String lastName();

	public abstract String street();

	public abstract String city();

	public abstract String email();

	public abstract String phone();

	public abstract Boolean member();

	public abstract Boolean updates();

	public abstract String comments();

	public boolean isMember() {
		return member() != null ? member().booleanValue() : false;
	}

	public Double cost(Event event) {
		return isMember() ? event.costMember() : event.costNonMember();
	}

	public boolean subscribeUpdates() {
		return updates() != null ? updates().booleanValue() : false;
	}

}
